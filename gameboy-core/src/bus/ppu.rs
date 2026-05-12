use crate::bus::cgb::Cgb;
use crate::bus::ppu::palette::Palette;
use crate::bus::ppu::registers::PpuRegisters;
use crate::util::Address;
use crate::{Cycles, Memory, Width};

mod cdma;
mod dma;
mod draw;
mod palette;
mod registers;

pub use cdma::Cdma;
pub use dma::Dma;

type Voam = Memory<0xA0>;

#[derive(Default, Clone)]
struct Vram {
    vram: Memory<{ Self::VRAM_BANK_SIZE * 2 }>,
    bank: bool,
}

impl Vram {
    pub const fn read(&self, index: usize) -> u8 {
        self.vram.read(index)
    }

    const fn map<const DMA: bool>(&self, regs: &PpuRegisters, address: &Address) -> Option<usize> {
        if DMA || (regs.lcdc_enabled() && regs.mode() != PpuRegisters::TRANSFER) {
            Some(self.bank(address.index() - 0x8000))
        } else {
            None
        }
    }

    pub const fn bank(&self, offset: usize) -> usize {
        offset + (self.bank as usize * Vram::VRAM_BANK_SIZE)
    }

    pub const fn write(&mut self, index: usize, value: u8) {
        self.vram.write(index, value);
    }
}

impl Vram {
    pub const VRAM_BANK_SIZE: usize = 0x2000;
}

#[derive(Default)]
pub struct Ppu {
    clock: usize,
    vram: Vram,
    voam: Voam,
    regs: registers::PpuRegisters,
    framebuffer: Memory<{ Self::SCREEN_WIDTH * Self::SCREEN_HEIGHT }, Pixel>,
    window_line: u8,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Pixel {
    pub rgb: [u8; 3],
}

impl Pixel {
    pub const fn monochrome(palette: u8, color: u8) -> Self {
        const fn palette_color(palette: u8, color: u8) -> u8 {
            let shift = (color & 0x03) * 2;
            (palette >> shift) & 0x03
        }

        match palette_color(palette, color) {
            0 => Self {
                rgb: [0xE0, 0xF8, 0xD0],
            },
            1 => Self {
                rgb: [0x88, 0xC0, 0x70],
            },
            2 => Self {
                rgb: [0x34, 0x68, 0x56],
            },
            3 => Self {
                rgb: [0x08, 0x18, 0x20],
            },
            _ => panic!("Invalid monochrome color"),
        }
    }

    pub const fn rgb(rgb: &[u8; 3]) -> Self {
        let r = rgb[0] as u16;
        let g = rgb[1] as u16;
        let b = rgb[2] as u16;
        Self {
            rgb: [
                ((r * 13 + g * 2 + b) >> 1) as u8,
                ((g * 3 + b) << 1) as u8,
                ((r * 3 + g * 2 + b * 11) >> 1) as u8,
            ],
        }
    }
}

impl std::ops::Deref for Pixel {
    type Target = [u8; 3];

    fn deref(&self) -> &Self::Target {
        &self.rgb
    }
}

impl Ppu {
    pub const SCREEN_WIDTH: usize = 160;
    pub const SCREEN_HEIGHT: usize = 144;

    pub const ADDRESS_VBK: Width = 0xFF4F; // CGB only, VRAM bank select

    const fn voam_map<const DMA: bool>(&self, address: &Address) -> Option<usize> {
        if DMA
            || (self.regs.lcdc_enabled()
                && matches!(self.mode(), PpuRegisters::VBLANK | PpuRegisters::HBLANK))
        {
            Some(address.index() - 0xFE00)
        } else {
            None
        }
    }

    pub const fn read<const DMA: bool>(&self, address: &Address, cgb: &Cgb) -> u8 {
        match address.value() {
            Self::ADDRESS_VBK => self.vram.bank as u8 | !1,
            0x8000..=0x9FFF => {
                if let Some(index) = self.vram.map::<DMA>(&self.regs, address) {
                    self.vram.read(index)
                } else {
                    u8::MAX
                }
            }
            0xFE00..=0xFE9F => {
                if let Some(index) = self.voam_map::<DMA>(address) {
                    self.voam.read(index)
                } else {
                    u8::MAX
                }
            }
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B | 0xFF68..=0xFF6C => self.regs.read(cgb, &address),
            _ => unreachable!(),
        }
    }

    pub const fn write<const DMA: bool>(&mut self, cgb: &Cgb, address: &Address, value: u8) {
        match address.value() {
            Self::ADDRESS_VBK => self.vram.bank = value & 1 != 0,
            0x8000..=0x9FFF => {
                if let Some(index) = self.vram.map::<DMA>(&self.regs, address) {
                    self.vram.write(index, value);
                }
            }
            0xFE00..=0xFE9F => {
                if let Some(index) = self.voam_map::<DMA>(address) {
                    self.voam.write(index, value);
                }
            }
            0xFF40..=0xFF45 | 0xFF47..=0xFF4B | 0xFF68..=0xFF6C => {
                if self.regs.write(cgb, &address, value) {
                    self.clock = 0;
                    self.window_line = 0;
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn cycle(&mut self, int: &mut u8, cgb: &Cgb, cycles: &Cycles) -> bool {
        if !self.regs.lcdc_enabled() {
            return false;
        }

        let mut render = false;
        self.clock += cycles.t();
        loop {
            match self.mode() {
                PpuRegisters::HBLANK if self.clock >= 204 => {
                    self.clock -= 204;
                    self.regs.update_ly(int, self.ly().wrapping_add(1));
                    if self.ly() == Self::SCREEN_HEIGHT as u8 {
                        self.regs.set_mode(int, PpuRegisters::VBLANK);
                        render = true;
                    } else {
                        self.regs.set_mode(int, PpuRegisters::OAM);
                    }
                }
                PpuRegisters::VBLANK if self.clock >= 456 => {
                    self.clock -= 456;
                    self.regs.update_ly(int, self.ly().wrapping_add(1));
                    if self.ly() >= Self::SCREEN_HEIGHT as u8 + 10 {
                        self.regs.update_ly(int, 0);
                        self.window_line = 0;
                        self.regs.set_mode(int, PpuRegisters::OAM);
                    }
                }
                PpuRegisters::OAM if self.clock >= 80 => {
                    self.clock -= 80;
                    self.regs.set_mode(int, PpuRegisters::TRANSFER);
                }
                PpuRegisters::TRANSFER if self.clock >= 172 => {
                    self.clock -= 172;
                    if self.ly() < Self::SCREEN_HEIGHT as u8 {
                        self.draw_dmg(cgb);
                    }
                    self.regs.set_mode(int, PpuRegisters::HBLANK);
                }
                _ => break,
            }
        }
        render
    }

    pub const fn ly(&self) -> u8 {
        self.regs.ly
    }

    fn draw_dmg(&mut self, cgb: &Cgb) {
        let y = self.ly() as u16;
        let bg_enable = self.regs.lcdc & 0x01 != 0;
        let sprite_enable = self.regs.lcdc & 0x02 != 0;
        let sprite_size = if self.regs.lcdc & 0x04 != 0 { 16 } else { 8 };
        let bg_map_base = if self.regs.lcdc & 0x08 != 0 {
            0x1C00
        } else {
            0x1800
        };
        let tile_data_base = if self.regs.lcdc & 0x10 != 0 {
            0x0000
        } else {
            0x1000
        };
        let window_enable = self.regs.lcdc & 0x20 != 0;
        let window_map_base = if self.regs.lcdc & 0x40 != 0 {
            0x1C00
        } else {
            0x1800
        };

        let mut window_drawn = false;
        for x in 0..Self::SCREEN_WIDTH as u16 {
            let mut final_color = None;
            let mut priority = draw::PrioType::Normal;
            if bg_enable {
                let mut use_window = false;
                let wx = self.regs.wx.wrapping_sub(7);
                if window_enable
                    && y >= self.regs.wy as u16
                    && x >= wx as u16
                    && self.regs.wx <= 166
                {
                    use_window = true;
                    window_drawn = true;
                }

                let (map_base, pixel_x, pixel_y) = if use_window {
                    let wx = self.regs.wx.wrapping_sub(7) as u16;
                    let px = x - wx;
                    let py = (self.window_line as u16).wrapping_sub(0);
                    (window_map_base, px, py)
                } else {
                    let px = x.wrapping_add(self.regs.scx as u16);
                    let py = y.wrapping_add(self.regs.scy as u16);
                    (bg_map_base, px, py)
                };

                let tile_x = (pixel_x / 8) & 31;
                let tile_y = (pixel_y / 8) & 31;

                let tile_index_addr = (map_base + tile_y * 32 + tile_x) as usize;
                let tile_index = self.vram.read(tile_index_addr);

                let mut tile_addr = if self.regs.lcdc & 0x10 != 0 {
                    tile_data_base + (tile_index as usize) * 16
                } else {
                    let signed = tile_index as i8 as i16;
                    (0x1000i16 + signed * 16) as usize
                };

                let (palnr, xflip, yflip, prio) = if cgb.enabled() {
                    let flags = self.vram.read(tile_index_addr + Vram::VRAM_BANK_SIZE);

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

                let line = pixel_y as usize % 8;
                let bit = if xflip {
                    pixel_x % 8
                } else {
                    7 - (pixel_x % 8)
                };

                let color_addr = match yflip {
                    false => tile_addr + (line * 2),
                    true => tile_addr + (14 - (line * 2)),
                };

                let lo = self.vram.read(color_addr);
                let hi = self.vram.read(color_addr + 1);

                let col = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);

                if cgb.enabled() {
                    priority = if col == 0 {
                        draw::PrioType::Color0
                    } else if prio {
                        draw::PrioType::PrioFlag
                    } else {
                        draw::PrioType::Normal
                    };
                }

                final_color = Some(if cgb.enabled() {
                    Pixel::rgb(&self.regs.bcp.color(palnr, col))
                } else {
                    Pixel::monochrome(self.regs.bgp, col)
                });
            }

            if priority == draw::PrioType::Normal {
                draw::draw_sprites(
                    &self.regs,
                    &self.vram,
                    &self.voam,
                    cgb,
                    &[draw::PrioType::Normal; Self::SCREEN_WIDTH],
                    self.framebuffer.as_slice_mut(),
                );
            }

            let ly = self.ly() as usize;
            self.framebuffer.as_slice_mut()[ly * Self::SCREEN_WIDTH + x as usize] =
                final_color.unwrap_or_default();
        }

        if window_drawn {
            self.window_line = self.window_line.wrapping_add(1);
        }
    }

    // fn draw(&mut self, cgb: &Cgb) {
    //     #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    //     enum PrioType {
    //         Color0,
    //         PrioFlag,
    //         Normal,
    //     }

    //     let mut bgprio = [PrioType::Normal; Self::SCREEN_WIDTH];

    //     let bg_enable = cgb.enabled() || self.lcdc & 0x01 != 0;

    //     // let wx_trigger = self.wx <= 166;
    //     // let winy = if  self.lcdc & 0x20 != 0 && self.wy_trigger && wx_trigger {
    //     //     self.wy_pos += 1;
    //     //     self.wy_pos
    //     // } else {
    //     //     -1
    //     // };

    //     let bg_map_base = if self.lcdc & 0x08 != 0 {
    //         0x1C00usize
    //     } else {
    //         0x1800
    //     };

    //     let tile_data_base = if self.lcdc & 0x10 != 0 {
    //         0x0000
    //     } else {
    //         0x1000
    //     };

    //     let window_map_base = if self.lcdc & 0x40 != 0 {
    //         0x1C00
    //     } else {
    //         0x1800
    //     };

    //     let wintiley = (self.wy as u16 >> 3) & 31;

    //     let bgy = self.scy.wrapping_add(self.ly());
    //     let bgtiley = (bgy as u16 >> 3) & 31;

    //     for x in 0..Self::SCREEN_WIDTH {
    //         let winx = -((self.wx as i32) - 7) + (x as i32);
    //         let bgx = self.scx as u32 + x as u32;

    //         let (tilemapbase, tiley, tilex, pixely, pixelx) = if self.wy >= 0 && winx >= 0 {
    //             (
    //                 window_map_base,
    //                 wintiley,
    //                 (winx as u16 >> 3),
    //                 self.wy as u16 & 0x07,
    //                 winx as u8 & 0x07,
    //             )
    //         } else if bg_enable {
    //             (
    //                 bg_map_base,
    //                 bgtiley,
    //                 (bgx as u16 >> 3) & 31,
    //                 bgy as u16 & 0x07,
    //                 bgx as u8 & 0x07,
    //             )
    //         } else {
    //             continue;
    //         };

    //         let tilenraddr = tilemapbase + (tiley * 32 + tilex) as usize;
    //         let tilenr: u8 = self.vram.read(tilenraddr);

    //         let (palnr, vram1, xflip, yflip, prio) = if cgb.enabled() {
    //             let flags = self.vram.read(tilenraddr + Self::VRAM_BANK_SIZE);
    //             (
    //                 flags & 0x07,
    //                 flags & (1 << 3) != 0,
    //                 flags & (1 << 5) != 0,
    //                 flags & (1 << 6) != 0,
    //                 flags & (1 << 7) != 0,
    //             )
    //         } else {
    //             (0, false, false, false, false)
    //         };

    //         let tileaddress = tile_data_base
    //             + (if tile_data_base == 0x0000 {
    //                 tilenr as u16
    //             } else {
    //                 (tilenr as i8 as i16 + 128) as u16
    //             }) * 16;

    //         let a0 = match yflip {
    //             false => tileaddress + (pixely * 2),
    //             true => tileaddress + (14 - (pixely * 2)),
    //         } as usize;

    //         let (b1, b2) = match vram1 {
    //             false => (self.vram.read(a0), self.vram.read(a0 + 1)),
    //             true => (
    //                 self.vram.read(a0 + Vram::VRAM_BANK_SIZE),
    //                 self.vram.read(a0 + 1 + Vram::VRAM_BANK_SIZE),
    //             ),
    //         };

    //         let xbit = match xflip {
    //             true => pixelx,
    //             false => 7 - pixelx,
    //         } as u32;
    //         let colnr = if b1 & (1 << xbit) != 0 { 1 } else { 0 }
    //             | if b2 & (1 << xbit) != 0 { 2 } else { 0 };

    //         bgprio[x] = if colnr == 0 {
    //             PrioType::Color0
    //         } else if prio {
    //             PrioType::PrioFlag
    //         } else {
    //             PrioType::Normal
    //         };

    //         // if cgb.enabled() {
    //         // self.framebuffer[x as usize] = Pixel::rgb(&self.bcpd[palnr as usize][colnr]);
    //         // } else {
    //         self.framebuffer[x as usize] = Pixel::monochrome(palnr, colnr as u8);
    //         // }
    //     }

    //     if self.regs.lcdc & 0x02 != 0 {
    //         let sprite_size = if self.regs.lcdc & 0x04 != 0 { 16 } else { 8 };
    //     }
    // }

    pub const fn clock(&self) -> usize {
        self.clock
    }

    pub const fn framebuffer(&self) -> &[Pixel] {
        self.framebuffer.as_slice()
    }

    pub const fn lcdc(&self) -> u8 {
        self.regs.lcdc
    }

    pub const fn stat(&self) -> u8 {
        self.regs.stat
    }

    pub const fn mode(&self) -> u8 {
        self.regs.mode()
    }
}
