pub mod bus;
mod cartridge;
pub mod cpu;
pub mod util;

pub use crate::cartridge::*;
pub use crate::util::*;

use self::cpu::CycleError;


#[derive(Default)]
pub struct GameboyColor {
    pub cpu: cpu::CPU,
    pub bus: bus::Bus,
}

/**
 * T-Cycles
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cycles(usize);

impl Cycles {
    pub const fn new(cycles: usize) -> Self {
        Self(cycles)
    }

    pub const fn t(&self) -> usize {
        self.0
    }

    pub const fn m(&self) -> usize {
        self.0 / 4
    }

    pub const fn split(self, double: bool, vram: Cycles) -> (Self, Self) {
        (Self(self.0 + vram.0), Self(self.0 * if double { 2 } else { 1 } + vram.0))
    }
}

impl PartialEq<usize> for Cycles {
    fn eq(&self, other: &usize) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<usize> for Cycles {
    fn partial_cmp(&self, other: &usize) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl std::ops::AddAssign for Cycles {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

pub struct GameboyCycle {
    pub cpu: cpu::CycleResult,
    pub render: bool,
}
impl GameboyColor {
    pub const CLOCK_SPEED: usize = 4194304;

    pub fn with_serial_callback(callback: Box<dyn FnMut(u8)>) -> Self {
        Self { bus: bus::Bus::with_serial_callback(callback), ..Self::default() }
    }

    pub fn load(data: impl AsRef<[u8]>) -> Box<dyn Cartridge + 'static> {
        cartridge::load(data)
    }

    pub fn cycle(
        &mut self,
        cart: &mut dyn Cartridge
    ) -> Result<GameboyCycle, CycleError> {
        let cpu = self.cpu.cycle(cart, &mut self.bus)?;
        self.bus
            .cycle(cart, &cpu)
            .map(|render| GameboyCycle { cpu, render })
            .map_err(|e| CycleError::Bus(self.cpu.pc(), e))
    }

    pub fn reset(&mut self, cart: &dyn Cartridge) {
        self.cpu.reset();
        self.bus.reset();
        self.cpu.load(cart);
        self.bus.load(cart);
    }

    pub fn frame_to_rgba(&self, output: &mut [u8]) {
        for (idx, pixel) in self.bus.ppu.framebuffer().iter().enumerate() {
            let base = idx * 4;
            output[base..base + 3].copy_from_slice(&**pixel);
            output[base + 3] = 0xFF;
        }
    }

    pub fn update_input(&mut self, button: Controls, pressed: bool) {
        self.bus.update_input(button, pressed);
    }

    pub fn handle_interrupts(&mut self) {}
}
