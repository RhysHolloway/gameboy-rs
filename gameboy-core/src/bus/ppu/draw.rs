use crate::bus::cgb;
use crate::bus::ppu::{Pixel, Ppu, PpuRegisters, Vram};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrioType {
    Color0,
    PrioFlag,
    Normal,
}

pub fn draw_background(
    regs: &super::PpuRegisters,
    vram: &super::Vram,
    cgb: &cgb::Cgb,
    window_line: &mut u8,
    line: &mut [Pixel],
) -> [PrioType; Ppu::SCREEN_WIDTH] {
    let y = regs.ly() as u16;
    let bg_enable = regs.lcdc & PpuRegisters::LCDC_BG != 0;
    // In CGB mode LCDC.0 controls BG/window priority, not BG/window visibility.
    let bg_display = cgb.enabled() || bg_enable;
    let bg_priority = !cgb.enabled() || bg_enable;
    let window_enable = regs.lcdc & PpuRegisters::LCDC_WINDOW != 0;

    let bg_map_base = if regs.lcdc & PpuRegisters::LCDC_BG_MAP != 0 {
        0x1C00
    } else {
        0x1800
    };
    let tile_data_base = if regs.lcdc & PpuRegisters::LCDC_TILE_DATA != 0 {
        0x0000
    } else {
        0x1000
    };
    let window_map_base = if regs.lcdc & PpuRegisters::LCDC_WINDOW_MAP != 0 {
        0x1C00
    } else {
        0x1800
    };

    let mut bgprio = [PrioType::Color0; Ppu::SCREEN_WIDTH];
    if bg_display || window_enable {
        let mut window_drawn = false;
        for x in 0..Ppu::SCREEN_WIDTH as u16 {
            let window_x = regs.wx as i16 - 7;
            let use_window =
                window_enable && y >= regs.wy as u16 && (x as i16) >= window_x && regs.wx <= 166;

            if !bg_display && !use_window {
                continue;
            }

            if use_window {
                window_drawn = true;
            }

            let (map_base, pixel_x, pixel_y) = if use_window {
                let px = (x as i16 - window_x) as u16;
                let py = (*window_line as u16).wrapping_sub(0);
                (window_map_base, px, py)
            } else {
                let px = x.wrapping_add(regs.scx as u16);
                let py = y.wrapping_add(regs.scy as u16);
                (bg_map_base, px, py)
            };

            let tile_x = (pixel_x / 8) & 31;
            let tile_y = (pixel_y / 8) & 31;

            let tile_index_addr = (map_base + tile_y * 32 + tile_x) as usize;
            let tile_index = vram.read(tile_index_addr);

            let mut tile_addr = if regs.lcdc & PpuRegisters::LCDC_TILE_DATA != 0 {
                tile_data_base + (tile_index as usize) * 16
            } else {
                let signed = tile_index as i8 as i16;
                (0x1000i16 + signed * 16) as usize
            };

            let (palnr, xflip, yflip, priority) = if cgb.enabled() {
                let flags = vram.read(tile_index_addr + Vram::VRAM_BANK_SIZE);

                if flags & (1 << 3) != 0 {
                    tile_addr += Vram::VRAM_BANK_SIZE;
                }
                (
                    flags & 0x07,
                    flags & (1 << 5) != 0,
                    flags & (1 << 6) != 0,
                    flags & (1 << 7) != 0,
                )
            } else {
                (0, false, false, false)
            };

            let tile_pixel_x = (pixel_x & 7) as usize;
            let tile_pixel_y = (pixel_y & 7) as usize;
            let source_x = if xflip {
                7 - tile_pixel_x
            } else {
                tile_pixel_x
            };
            let source_y = if yflip {
                7 - tile_pixel_y
            } else {
                tile_pixel_y
            };
            let color_addr = tile_addr + source_y * 2;
            let bit = 7 - source_x;

            let lo = vram.read(color_addr);
            let hi = vram.read(color_addr + 1);

            let col = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);

            bgprio[x as usize] = if !bg_priority || col == 0 {
                PrioType::Color0
            } else if cgb.enabled() && priority {
                PrioType::PrioFlag
            } else {
                PrioType::Normal
            };

            line[x as usize] = if cgb.enabled() {
                Pixel::rgb(&regs.bcp.color(palnr, col))
            } else {
                Pixel::monochrome(regs.bgp, col)
            };
        }
        if window_drawn {
            *window_line = window_line.wrapping_add(1);
        }
    }
    return bgprio;
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
    let sprite_enable = regs.lcdc & PpuRegisters::LCDC_OBJ != 0;

    if sprite_enable {
        let sprite_size = if regs.lcdc & PpuRegisters::LCDC_OBJ_SIZE != 0 {
            16
        } else {
            8
        };

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
