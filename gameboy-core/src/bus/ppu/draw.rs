use crate::bus::cgb;
use crate::bus::ppu::{Pixel, Ppu, Vram};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrioType {
    Color0,
    PrioFlag,
    Normal,
}

#[derive(Clone, Copy, Debug)]
struct Sprite {
    x: i16,
    y: i16,
    tile: u8,
    attrs: u8,
    oam_index: usize,
}

pub fn draw_sprites(
    regs: &super::PpuRegisters,
    vram: &super::Vram,
    voam: &super::Voam,
    cgb: &cgb::Cgb,
    bgprio: &[PrioType; Ppu::SCREEN_WIDTH],
    framebuffer: &mut [Pixel],
) {
    let sprite_enable = regs.lcdc & 0x02 != 0;

    if sprite_enable {
        let sprite_size = if regs.lcdc & 0x04 != 0 { 16 } else { 8 };

        let mut sprites: [Sprite; 10] = [Sprite {
            x: 0,
            y: 0,
            tile: 0,
            attrs: 0,
            oam_index: 0,
        }; 10];

        let mut sprite_count = 0;
        for i in 0..40 {
            let base = i as usize * 4;
            let sy = voam.read(base) as i16 - 16;
            let sx = voam.read(base + 1) as i16 - 8;
            let tile = voam.read(base + 2);
            let attrs = voam.read(base + 3);
            if regs.ly() as i16 >= sy && (regs.ly() as i16) < sy + sprite_size && sprite_count < 10
            {
                sprites[sprite_count] = Sprite {
                    x: sx,
                    y: sy,
                    tile,
                    attrs,
                    oam_index: i,
                };
                sprite_count += 1;
            }
        }

        if sprite_count > 0 {
            sprites[..sprite_count].sort_unstable_by(|a, b| {
                if !cgb.enabled() && a.x != b.x {
                    return b.x.cmp(&a.x);
                }
                return b.oam_index.cmp(&a.oam_index);
            });

            for sprite in &sprites[..sprite_count] {
                if sprite.x < -7 || sprite.x >= (Ppu::SCREEN_WIDTH as i16) {
                    continue;
                }

                let xflip: bool = sprite.attrs & (1 << 5) != 0;
                let yflip: bool = sprite.attrs & (1 << 6) != 0;
                let belowbg: bool = sprite.attrs & (1 << 7) != 0;
                let c_palnr = sprite.attrs & 0x07;
                let d_pal = if sprite.attrs & (1 << 4) != 0 {
                    regs.obp1
                } else {
                    regs.obp0
                };
                let c_vram1: bool = sprite.attrs & (1 << 3) != 0;

                let tiley: u16 = if yflip {
                    (sprite_size - 1 - (regs.ly() as i16 - sprite.y)) as u16
                } else {
                    (regs.ly() as i16 - sprite.y) as u16
                };

                let tile = if sprite_size == 16 {
                    sprite.tile & 0xFE
                } else {
                    sprite.tile
                };
                let tile_address = tile as usize * 16
                    + tiley as usize * 2
                    + if c_vram1 && cgb.enabled() {
                        Vram::VRAM_BANK_SIZE
                    } else {
                        0
                    };

                let (b1, b2) = (vram.read(tile_address), vram.read(tile_address + 1));

                'xloop: for x in 0..8 {
                    if sprite.x + x < 0 || sprite.x + x >= (Ppu::SCREEN_WIDTH as i16) {
                        continue;
                    }

                    let xbit = 1 << (if xflip { x } else { 7 - x } as u32);
                    let colnr =
                        (if b1 & xbit != 0 { 1 } else { 0 }) | (if b2 & xbit != 0 { 2 } else { 0 });

                    if colnr == 0 {
                        continue;
                    }

                    if belowbg && bgprio[(sprite.x + x) as usize] != PrioType::Color0 {
                        continue 'xloop;
                    } else if cgb.enabled() {
                        if bgprio[(sprite.x + x) as usize] == PrioType::PrioFlag {
                            continue 'xloop;
                        }

                        framebuffer[(sprite.x + x) as usize] =
                            Pixel::rgb(&regs.ocp.color(c_palnr, colnr));
                    } else {
                        framebuffer[(sprite.x + x) as usize] = Pixel::monochrome(d_pal, colnr);
                    }
                }
            }
        }
    }
}
