mod apu;
mod cdma;
mod cgb;
mod dma;
mod interrupts;
mod ir;
mod joypad;
mod ppu;
mod serial;
mod timer;
mod wram;

use crate::cpu::CycleResult;
use crate::util::{Address, Controls};
use crate::{Cartridge, Memory, Width};
pub use interrupts::*;
pub use ppu::*;
pub use serial::SerialCallback;
pub use apu::AudioCallback;

pub type Callback<T> = Option<alloc::boxed::Box<dyn FnMut(T) + 'static>>;

#[derive(Default)]
pub struct Bus {
    wram: wram::Wram,
    pub ppu: ppu::Ppu,
    pub timer: timer::Timer,
    hram: Memory<0x7F>,
    joypad: joypad::Joypad,
    pub dma: dma::Dma,
    cdma: cdma::Cdma,
    ir: ir::Ir,
    pub apu: apu::APU,
    pub serial: serial::SerialState,
    pub interrupts: interrupts::Interrupts,
    pub cgb: cgb::Cgb,
}

impl Bus {
    const WRAM_START: usize = 0xC000;
    const ECHO_START: usize = 0xE000;
    const HRAM_START: usize = 0xFF80;

    pub fn set_serial_callback(&mut self, callback: serial::SerialCallback) {
        self.serial.callback = callback;
    }

    pub fn set_audio_callback(&mut self, callback: apu::AudioCallback) {
        self.apu.callback = callback;
    }

    pub fn load(&mut self, cart: &dyn Cartridge) {
        self.cgb.set_enabled(cart);
    }

    pub fn read<const DMA: bool>(&self, cart: &dyn Cartridge, address: Address) -> u8 {
        match !DMA && self.dma.is_active() {
            false => match address.value() {
                0x0000..=0x7FFF | 0xA000..=0xBFFF => cart.read(address),
                0x8000..=0x9FFF
                | 0xFE00..=0xFE9F
                | 0xFF40..=0xFF45
                | 0xFF47..=0xFF4B
                | 0xFF4F
                | 0xFF68..=0xFF6C => self.ppu.read::<DMA>(&address, &self.cgb),
                0xC000..=0xDFFF => self.wram.read(address.sub(Self::WRAM_START)),
                0xE000..=0xFDFF => self.wram.read(address.sub(Self::ECHO_START)), // wram echo during DMA
                0xFF00 => self.joypad.read(),
                0xFF01 | 0xFF02 => self.serial.read(&address),
                0xFF04..=0xFF07 => self.timer.read(&address),
                0xFF0F => self.interrupts.i(),
                0xFF10..=0xFF26 | 0xFF30..=0xFF3F => self.apu.read(&address),
                0xFF46 => self.dma.read(),
                0xFF4C | 0xFF4D | 0xFF72..=0xFF75 => self.cgb.read_mapped(&address),
                0xFF51..=0xFF55 if self.cgb.enabled() => self.cdma.read(&address),
                0xFF56 => self.ir.read(),
                0xFF70 if self.cgb.enabled() => self.wram.read_bank(),
                0xFF80..=0xFFFE => self.hram.read(address.index() - Self::HRAM_START),
                0xFFFF => self.interrupts.ie(),
                _ => u8::MAX,
            },
            true => match address.value() {
                0xFF80..=0xFFFE => self.hram.read(address.index() - Self::HRAM_START),
                _ => return u8::MAX,
            },
        }
    }

    pub fn write<const DMA: bool>(
        &mut self,
        cart: &mut dyn Cartridge,
        address: Address,
        value: u8,
    ) {
        match !DMA && self.dma.is_active() {
            false => match address.value() {
                0x0000..=0x7FFF | 0xA000..=0xBFFF => cart.write(address, value),
                0x8000..=0x9FFF
                | 0xFE00..=0xFE9F
                | 0xFF40..0xFF46
                | 0xFF47..=0xFF4B
                | 0xFF4F
                | 0xFF68..=0xFF6C => {
                    self.ppu.write::<DMA>(&self.cgb, &address, value);
                }
                0xC000..=0xDFFF => self.wram.write(address.sub(Self::WRAM_START), value),
                0xE000..=0xFDFF => self.wram.write(address.sub(Self::ECHO_START), value),
                0xFF00 => self.joypad.write(value),
                0xFF01 | 0xFF02 => self.serial.write(&address, value),
                0xFF04..=0xFF07 => self.timer.write(&address, value),
                0xFF10..=0xFF26 | 0xFF30..=0xFF3F => self.apu.write(&address, value),
                0xFF0F => self.interrupts.i = value & 0x1F,
                0xFF46 => self.dma.write(value),
                0xFF4C | 0xFF4D | 0xFF72..=0xFF75 => self.cgb.write_mapped(&address, value),
                0xFF51..=0xFF55 if self.cgb.enabled() => self.cdma.write(&address, value),
                0xFF56 => self.ir.write(value),
                0xFF70 if self.cgb.enabled() => self.wram.write_bank(value),
                0xFF80..=0xFFFE => self.hram.write(address.sub(Self::HRAM_START), value),
                0xFFFF => self.interrupts.ie = value,
                _ => {}
            },
            true => match address.value() {
                0xFF80..=0xFFFE => self.hram.write(address.sub(Self::HRAM_START), value),
                _ => {}
            },
        }
    }

    pub fn read_word<const DMA: bool>(&self, cart: &dyn Cartridge, address: Address) -> Width {
        Width::from_le_bytes([
            self.read::<DMA>(cart, address),
            self.read::<DMA>(cart, address + 1),
        ])
    }

    pub fn write_word(&mut self, cart: &mut dyn Cartridge, address: Address, value: u16) {
        let bytes = Width::to_le_bytes(value);
        self.write::<false>(cart, address, bytes[0]);
        self.write::<false>(cart, address + 1, bytes[1]);
    }

    pub(super) fn cycle(&mut self, cart: &dyn Cartridge, result: &mut CycleResult) -> bool {
        self.cdma_cycle(result, cart);

        let fast = &result.cycles;
        let slow = *fast / (self.cgb.double_speed() as usize + 1);

        self.timer.cycle(&mut self.interrupts, &fast);
        let render = self.ppu.cycle(&mut self.interrupts.i, &self.cgb, &slow);

        self.cycle_dma(&fast, cart);

        self.serial.cycle(&mut self.interrupts.i, &fast);
        self.apu.cycle(&slow);
        render
    }

    pub fn reset(&mut self) {
        let serial_callback = self.serial.callback.take();
        let audio_callback = self.apu.callback.take();
        *self = Self::default();
        self.set_serial_callback(serial_callback);
        self.set_audio_callback(audio_callback);
    }

    pub fn update_input(&mut self, button: Controls, pressed: bool) {
        self.joypad.update(&mut self.interrupts, (button, pressed));
    }

    pub(super) fn stop(&mut self) {
        self.timer.reset_div();
        if !self.cgb.disarm() {
            self.interrupts.set_stop(true);
        }
    }
}
