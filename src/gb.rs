mod bus;
mod cpu;
mod debugger;
mod util;

pub use debugger::Debugger;

use self::cpu::CycleError;

pub struct GameboyColor {
    pub cpu: cpu::CPU,
    bus: bus::Bus,
    cycles: Cycles,
}

/**
 * T-Cycles
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cycles(usize);

impl Cycles {

    pub fn new(cycles: usize) -> Self {
        Self(cycles)
    }

    pub fn t(&self) -> usize {
        self.0
    }

    pub fn m(&self) -> usize {
        self.0 / 4
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

impl<'a> std::ops::Div<u8> for &'a Cycles {
    type Output = Cycles;

    fn div(self, rhs: u8) -> Self::Output {
        Cycles(self.0 / rhs as usize)
    }
}

pub struct GameboyCycle {
    pub cpu: cpu::CycleExecution,
    pub render: bool,
}

impl GameboyColor {
    pub const CLOCK_SPEED: usize = 4194304;

    pub fn new(rom: Vec<u8>) -> Self {
        Self {
            cpu: cpu::CPU::new(),
            bus: bus::Bus::new(rom),
            cycles: Cycles(0),
        }
    }

    pub fn cycle(&mut self) -> Result<GameboyCycle, CycleError> {
        let cpu = self.cpu.cycle(&mut self.bus)?;
        self.cycles.0 += cpu.cycles.0; 
        self.bus.cycle(&cpu.cycles).map(|render| GameboyCycle { cpu, render }).map_err(|e| CycleError::Bus(self.cpu.pc(), e))
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
        self.bus.reset();
    }

    pub fn cycles(&self) -> usize {
        self.cycles.0
    }
        
    pub fn frame_to_rgba(&self, output: &mut [u8]) {

        const DEFAULT_PALETTE: [[u8; 4]; 4] = [
            [0xE0, 0xF8, 0xD0, 0xFF],
            [0x88, 0xC0, 0x70, 0xFF],
            [0x34, 0x68, 0x56, 0xFF],
            [0x08, 0x18, 0x20, 0xFF],
        ];

        for (idx, shade) in self.bus.ppu.framebuffer().iter().enumerate() {
            let shade = (*shade & 0x03) as usize;
            let base = idx * 4;
            output[base..base+4].copy_from_slice(&DEFAULT_PALETTE[shade]);
        }
        
    }

    pub fn handle_interrupts(&mut self) {}

    pub fn title(&self) -> &str {
        self.bus.cartridge.title()
    }
}
