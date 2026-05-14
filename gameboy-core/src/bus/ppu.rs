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
        let mut line = [if cgb.enabled() {
            Pixel::rgb(&self.regs.bcp.color(0, 0))
        } else {
            Pixel::monochrome(self.regs.bgp, 0)
        }; Ppu::SCREEN_WIDTH];

        let bgprio = draw::draw_background(
            &self.regs,
            &self.vram,
            cgb,
            &mut self.window_line,
            &mut line,
        );

        draw::draw_sprites(&self.regs, &self.vram, &self.voam, cgb, &bgprio, &mut line);

        let start = self.ly() as usize * Self::SCREEN_WIDTH;
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
