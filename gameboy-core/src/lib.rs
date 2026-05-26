#![no_std]

extern crate alloc;

pub mod bus;
mod cartridge;
pub mod cpu;
pub mod util;

use core::cmp::Ordering;
use core::ops::{AddAssign, Div, Mul};
use alloc::boxed::Box;

use crate::bus::Framebuffer;
pub use crate::cartridge::*;
pub use crate::util::*;

#[derive(Default)]
pub struct GameboyColor {
    pub cpu: cpu::CPU,
    pub bus: bus::Bus,
}

/**
 * T-Cycles
 */
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
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
}

impl PartialEq<usize> for Cycles {
    fn eq(&self, other: &usize) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<usize> for Cycles {
    fn partial_cmp(&self, other: &usize) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl AddAssign for Cycles {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Mul<usize> for Cycles {
    type Output = Self;

    fn mul(self, rhs: usize) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div<usize> for Cycles {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        Self(self.0 / rhs)
    }
}

pub struct GameboyCycle {
    pub cpu: cpu::CycleResult,
    pub render: bool,
}
impl GameboyColor {
    pub const CLOCK_SPEED: usize = 4194304;

    pub fn load(data: impl AsRef<[u8]>) -> Result<Box<dyn Cartridge + 'static>, CartridgeError> {
        cartridge::load(data)
    }

    pub fn cycle(&mut self, cart: &mut dyn Cartridge) -> GameboyCycle {
        let mut cpu = self.cpu.cycle(cart, &mut self.bus);
        let render = self.bus.cycle(cart, &mut cpu);
        GameboyCycle { cpu, render }
    }

    pub fn reset(&mut self, cart: &dyn Cartridge) {
        self.cpu.reset();
        self.bus.reset();
        self.cpu.load(cart);
        self.bus.load(cart);
    }

    pub fn update_input(&mut self, button: Controls, pressed: bool) {
        self.bus.update_input(button, pressed);
    }

    pub fn handle_interrupts(&mut self) {}
    
    pub const fn framebuffer(&self) -> &Framebuffer {
        self.bus.ppu.framebuffer()
    }
}
