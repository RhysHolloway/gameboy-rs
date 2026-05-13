use crate::bus::cgb::Cgb;
use crate::bus::ppu::palette::Palette;
use crate::util::Address;
use crate::{Cycles, Memory, Width};

mod draw;
mod memory;
mod palette;
mod pixel;
mod registers;

pub use memory::*;
pub use pixel::Pixel;
pub use registers::PpuRegisters;

#[derive(Default)]
pub struct Ppu {
    clock: usize,
    pub vram: Vram,
    pub voam: Voam,
    regs: registers::PpuRegisters,
    framebuffer: Memory<{ Self::SCREEN_WIDTH * Self::SCREEN_HEIGHT }, Pixel>,
    window_line: u8,
}

impl Ppu {
    pub const SCREEN_WIDTH: usize = 160;
    pub const SCREEN_HEIGHT: usize = 144;

    pub const ADDRESS_VBK: Width = 0xFF4F; // CGB only, VRAM bank select

    const fn voam_map<const DMA: bool>(&self, address: &Address) -> Option<usize> {
        if DMA || matches!(self.mode(), PpuRegisters::VBLANK | PpuRegisters::HBLANK) {
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
                        self.draw(cgb);
                    }
                    self.regs.set_mode(int, PpuRegisters::HBLANK);
                }
                _ => break,
            }
        }
        render
    }

    pub const fn vram_slice(&self) -> &[u8] {
        self.vram.memory.as_slice()
    }

    pub const fn ly(&self) -> u8 {
        self.regs.ly
    }

    fn draw(&mut self, cgb: &Cgb) {
        let y = self.ly() as u16;
        let bg_enable = self.regs.lcdc & 0x01 != 0;
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

        let backdrop = if cgb.enabled() {
            Pixel::rgb(&self.regs.bcp.color(0, 0))
        } else {
            Pixel::monochrome(self.regs.bgp, 0)
        };
        let mut line = [backdrop; Self::SCREEN_WIDTH];
        let mut bgprio = [draw::PrioType::Color0; Self::SCREEN_WIDTH];
        if bg_enable {
            let mut window_drawn = false;
            for x in 0..Self::SCREEN_WIDTH as u16 {
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

                let (palnr, xflip, yflip, priority) = if cgb.enabled() {
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

                let tile_line = pixel_y as usize % 8;
                let bit = if xflip {
                    pixel_x % 8
                } else {
                    7 - (pixel_x % 8)
                };

                let color_addr = match yflip {
                    false => tile_addr + (tile_line * 2),
                    true => tile_addr + (14 - (tile_line * 2)),
                };

                let lo = self.vram.read(color_addr);
                let hi = self.vram.read(color_addr + 1);

                let col = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);

                bgprio[x as usize] = if col == 0 {
                    draw::PrioType::Color0
                } else if cgb.enabled() && priority {
                    draw::PrioType::PrioFlag
                } else {
                    draw::PrioType::Normal
                };

                line[x as usize] = if cgb.enabled() {
                    Pixel::rgb(&self.regs.bcp.color(palnr, col))
                } else {
                    Pixel::monochrome(self.regs.bgp, col)
                };
            }
            if window_drawn {
                self.window_line = self.window_line.wrapping_add(1);
            }
        }

        draw::draw_sprites(&self.regs, &self.vram, &self.voam, cgb, &bgprio, &mut line);

        let ly = self.ly() as usize;
        let start = ly * Self::SCREEN_WIDTH;
        self.framebuffer.as_slice_mut()[start..start + Self::SCREEN_WIDTH].copy_from_slice(&line);
    }

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
