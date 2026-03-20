mod cgb;
mod interrupts;
mod joypad;
mod ppu;
mod serial;
mod timer;
mod wram;
mod ir;

use crate::{Cartridge, Width};
use crate::cpu::CycleResult;
use crate::util::{Address, Controls, MemoryError, OffsetMemory};
pub use interrupts::*;

pub struct Bus {
    wram: wram::Wram,
    pub ppu: ppu::Ppu,
    pub timer: timer::Timer,
    hram: OffsetMemory<0xFF80, 0x7F>,
    joypad: joypad::Joypad,
    dma: ppu::Dma,
    cdma: ppu::Cdma,
    ir: ir::Ir,
    pub serial: serial::SerialState,
    pub interrupts: interrupts::Interrupts,
    pub cgb: cgb::Cgb,
}

impl Default for Bus {
    fn default() -> Self {
        Self {
            hram: OffsetMemory::new("High RAM"),
            wram: wram::Wram::default(),
            interrupts: Default::default(),
            ppu: ppu::Ppu::default(),
            timer: timer::Timer::default(),
            serial: serial::SerialState::default(),
            joypad: joypad::Joypad::default(),
            dma: ppu::Dma::default(),
            ir: ir::Ir::default(),
            cdma: ppu::Cdma::default(),
            cgb: cgb::Cgb::default(),
        }
    }
}

pub type Bank = u8;

impl Bus {

    pub fn with_serial_callback(callback: Box<dyn FnMut(u8)>) -> Self {
        Self { serial: serial::SerialState::with_callback(callback), ..Self::default() }
    }

    pub fn load(&mut self, cart: &dyn Cartridge) {
        self.cgb.set_enabled(cart);
    }

    pub fn read<const DMA: bool>(&self, cart: &dyn Cartridge, address: Address) -> Result<u8, MemoryError> {
        Ok(match address.value() {
            0x0000..=0x7FFF | 0xA000..=0xBFFF => cart.read(address)?,
            0x8000..=0x9FFF => self.ppu.read_vram(address)?, // video ram
            0xC000..=0xDFFF => self.wram.read(address - 0xC000)?, // wram
            0xE000..=0xFDFF => self.wram.read(address - 0xE000)?, // wram echo during DMA
            0xFE00..=0xFE9F => self.ppu.read_voam::<DMA>(address)?, // object attribute memory
            // 0xFEA0..=0xFEFF => self.cgb.=, // unusable
            // 0xFF40..=0xFF4B => self.ppu.read_reg(address),
            0xFF00 => self.joypad.read(),
            0xFF01 | 0xFF02 => self.serial.read(&address),
            0xFF04..=0xFF07 => self.timer.read(&address),
            0xFF0F => self.interrupts.i,
            0xFF10..=0xFF26 => 0xFF, // self.audio.read(address)?,
            0xFF38..=0xFF3F => 0xFF, // self.audio.read(address)?,
            0xFF40..0xFF46 | 0xFF47..=0xFF4B | 0xFF4F | 0xFF68..=0xFF6C => self.ppu.read_reg(&self.cgb, &address)?,
            0xFF46 => self.dma.read(),
            0xFF4C | 0xFF4D | 0xFF51..=0xFF55 => self.cgb.read_mapped(&address)?,
            0xFF56 => self.ir.read(),
            0xFF70 => self.wram.read_bank(),
            0xFF80..=0xFFFE => self.hram.read_mapped(address)?,
            0xFFFF => self.interrupts.ie,
            _ => return Err(MemoryError::Read("Inaccessible", address.index())),
        })
    }

    pub const fn dma_active(&self) -> bool {
        self.dma.is_active() || self.cdma.is_active(&self.ppu)
    }

    pub fn write<const DMA: bool>(
        &mut self,
        cart: &mut dyn Cartridge,
        address: Address,
        value: u8,
    ) -> Result<(), MemoryError> {
        if self.dma_active() && address.value() < 0xFF80 {
            return Ok(()); // During DMA, only allow writes to HRAM
        }
        match address.value() {
            0x0000..=0x7FFF  |0xA000..=0xBFFF  => cart.write(address, value)?,
            0x8000..=0x9FFF => self.ppu.write_vram(address, value)?,
            0xC000..=0xDFFF => self.wram.write(address - 0xC000, value)?,
            0xE000..=0xFDFF => self.wram.write(address - 0xE000, value)?, // wram echo during DMA
            0xFE00..=0xFE9F => self.ppu.write_voam::<DMA>(address, value)?,
            0xFF00 => self.joypad.write(value),
            0xFF01 | 0xFF02 => self.serial.write(&address, value),
            0xFF04..=0xFF07 => self.timer.write(&address, value),
            0xFF10..=0xFF26 => (), // self.audio.write(address, value)?,
            0xFF30..=0xFF3F => (), // self.audio.write(address, value)?,
            0xFF0F => self.interrupts.i = value & 0x1F,
            0xFF40..0xFF46 | 0xFF47..=0xFF4B | 0xFF4F | 0xFF68..=0xFF6C => {
                self.ppu.write_reg(&self.cgb, &address, value)?
            }
            0xFF46 => self.dma.write(value), // OAM DMA
            0xFF4C | 0xFF4D | 0xFF51..=0xFF55 => self.cgb.write_mapped(&address, value)?,
            // 0xFF00..=0xFF7F => self.io.write(address - 0xFF00, value).map_err(From::from),
            0xFF56 => self.ir.write(value),
            0xFF70 => self.wram.write_bank(value),
            0xFF80..=0xFFFE => self.hram.write_mapped(address, value)?,
            0xFFFF => self.interrupts.ie = value,
            _ => {
                return Err(MemoryError::Write(
                    "Inaccessible",
                    address.index(),
                ));
            }
        }
        Ok(())
    }

    pub fn read_word<const DMA: bool>(
        &self,
        cart: &dyn Cartridge,
        address: Address,
    ) -> Result<Width, MemoryError> {
        Ok(Width::from_le_bytes([
            self.read::<DMA>(cart, address)?,
            self.read::<DMA>(cart, address + 1)?,
        ]))
    }

    pub fn write_word(
        &mut self,
        cart: &mut dyn Cartridge,
        address: Address,
        value: u16,
    ) -> Result<(), MemoryError> {
        let bytes = Width::to_le_bytes(value);
        self.write::<false>(cart, address, bytes[0])?;
        self.write::<false>(cart, address + 1, bytes[1])?;
        Ok(())
    }

    pub(super) fn cycle(
        &mut self,
        cart: &dyn Cartridge,
        cpu: &CycleResult,
    ) -> Result<bool, MemoryError> {
        let (slow, fast) = cpu.cycles.split(self.cgb.double_speed(), self.cdma.cycle(&cpu, cart, self)?);
        self.timer.cycle(&mut self.interrupts, &fast);
        let render = self.ppu.cycle(&mut self.interrupts.i, &self.cgb, &slow)?;
        self.dma.cycle(&slow, cart, self)?;
        self.serial.cycle(&mut self.interrupts.i, &fast);
        Ok(render)
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update_input(&mut self, button: Controls, pressed: bool) {
        self.joypad.update(&mut self.interrupts, (button, pressed));
    }
}