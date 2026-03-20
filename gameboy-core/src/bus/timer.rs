use crate::bus::Interrupts;
use crate::{Address, Cycles};

#[derive(Default)]
pub struct Timer {
    div: u8,
    tima: u8,
    tma: u8,
    tac: u8,
    internalcnt: u32,
    internaldiv: u32,
}

impl Timer {
    pub const DIV: Address = Address::new(0xFF04);
    pub const TIMA: Address = Address::new(0xFF05);
    pub const TMA: Address = Address::new(0xFF06);
    pub const TAC: Address = Address::new(0xFF07);

    const INTERRUPT_BIT: u8 = 0b100;
    const TAC_ENABLE_BIT: u8 = 0b100;
    const TAC_STEP_BITS: u8 = 0b11;

    pub(crate) const fn reset_div(&mut self) {
        self.div = 0;
        self.internaldiv = 0;
    }

    pub(super) const fn write(&mut self, a: &Address, v: u8) {
        match a {
            &Self::DIV => self.reset_div(),
            &Self::TIMA => self.tima = v,
            &Self::TMA => self.tma = v,
            &Self::TAC => self.tac = v,
            _ => unreachable!(),
        };
    }

    pub(super) const fn read(&self, a: &Address) -> u8 {
        match a {
            &Self::DIV => self.div,
            &Self::TIMA => self.tima,
            &Self::TMA => self.tma,
            &Self::TAC => 0xF8 | self.tac,
            _ => unreachable!(),
        }
    }

    pub fn cycle(&mut self, i: &mut Interrupts, cycles: &Cycles) {
        self.internaldiv += cycles.t() as u32;
        while self.internaldiv >= 256 {
            self.div = self.div.wrapping_add(1);
            self.internaldiv -= 256;
        }

        if self.enabled() {
            self.internalcnt += cycles.t() as u32;

            let step = self.step();

            while self.internalcnt >= step {
                self.tima = self.tima.wrapping_add(1);
                if self.tima == 0 {
                    self.tima = self.tma;
                    i.i |= Self::INTERRUPT_BIT;
                }
                self.internalcnt -= step;
            }
        }
    }

    const fn step(&self) -> u32 {
        match self.tac & Self::TAC_STEP_BITS {
            1 => 16,
            2 => 64,
            3 => 256,
            _ => 1024,
        }
    }

    const fn enabled(&self) -> bool {
        self.tac & Self::TAC_ENABLE_BIT != 0
    }

    pub const fn div(&self) -> u8 {
        self.div
    }

    pub const fn tima(&self) -> u8 {
        self.tima
    }

    pub const fn tma(&self) -> u8 {
        self.tma
    }

    pub const fn tac(&self) -> u8 {
        self.tac
    }
}
