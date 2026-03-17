use crate::gb::Cycles;
use crate::gb::util::{Address, MemoryError};

pub struct Timer {
    counter: u8,
    div: u8,
    tima: u8,
    tma: u8,
    tac: u8,
    tima_cooldown: u8,
}

impl Timer {

    pub const LOCATION: &'static str = "Timer";

    pub const DIV: u16 = 0xFF04;
    pub const TIMA: u16 = 0xFF05;
    pub const TMA: u16 = 0xFF06;
    pub const TAC: u16 = 0xFF07;

    pub const INTERRUPT_BIT: u8 = 0b00000100;

    const TAC_ENABLE_BIT: u8 = 0b00000010;
    const TIMA_COOLDOWN_OVERFLOW: u8 = 4;

    pub fn new() -> Self {
        Self {
            counter: 0,
            div: 0,
            tima: 0,
            tma: 0,
            tac: 0,
            tima_cooldown: 0,
        }
    }

    pub(super) fn write(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        match address.0 {
            Self::DIV => self.div = 0,
            Self::TIMA => self.tima = value,
            Self::TMA => {
                self.tma = value;
                self.tima_cooldown = 0;
            }
            Self::TAC => self.tac = value & 0b111,
            _ => return Err(MemoryError::write(Self::LOCATION, address)),
        }
        Ok(())
    }

    pub(super) fn read(&self, address: Address) -> Result<u8, MemoryError> {
        Ok(match address.0 {
            Self::DIV => self.div,
            Self::TIMA => self.tima,
            Self::TMA => self.tma,
            Self::TAC => self.tac | 0b11111000,
            _ => return Err(MemoryError::read(Self::LOCATION, address)),
        })
    }

    pub(super) fn cycle(&mut self, int: &mut u8, cycles: &Cycles) {
        for _ in 0..cycles.0 {
            let (counter, overflow) = self.counter.overflowing_add(1);
            self.counter = counter;
            if !overflow {
                continue;
            }

            let old_bit = self.tima_active();
            self.div = self.div.wrapping_add(1);
            let new_bit = self.tima_active();
            let enabled = (self.tac & Self::TAC_ENABLE_BIT) != 0;

            if self.tima_cooldown != 0 {
                self.tima_cooldown -= 1;
                if self.tima_cooldown == 0 {
                    self.tima = self.tma;
                    *int |= Self::INTERRUPT_BIT;
                }
            } else if enabled & old_bit & !new_bit {
                let (new_tima, overflow) = self.tima.overflowing_add(1);
                self.tima = new_tima;
                if overflow {
                    self.tima_cooldown = Self::TIMA_COOLDOWN_OVERFLOW;
                }
            }
        }
    }

    fn timer_enabled(&self) -> bool {
        self.tac & 0x04 != 0
    }

    fn timer_active(&self) -> bool {
        self.timer_enabled() && self.tima_active()
    }

    fn timer_bit(&self) -> u16 {
        match self.tac & 0x03 {
            0b00 => 1u16 << 9,
            0b01 => 1u16 << 3,
            0b10 => 1u16 << 5,
            0b11 => 1u16 << 7,
            _ => unreachable!(),
        }
    }

    fn tima_active(&self) -> bool {
        self.div as u16 & self.timer_bit() != 0
    }

    pub fn div(&self) -> u8 {
        self.div
    }

    pub fn tima(&self) -> u8 {
        self.tima
    }

    pub fn tma(&self) -> u8 {
        self.tma
    }

    pub fn tac(&self) -> u8 {
        self.tac
    }


}