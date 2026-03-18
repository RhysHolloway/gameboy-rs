use crate::Cycles;
use crate::util::Address;

#[derive(Default)]
pub struct Timer {
    div: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    overflow: bool,
}

impl Timer {
    pub const DIV: Address = Address::new(0xFF04);
    pub const TIMA: Address = Address::new(0xFF05);
    pub const TMA: Address = Address::new(0xFF06);
    pub const TAC: Address = Address::new(0xFF07);

    const INTERRUPT_BIT: u8 = 0b100;
    const TAC_ENABLE_BIT: u8 = 0b100;
    const TAC_FREQ_BITS: u8 = 0b11;

    pub(super) const fn write(&mut self, address: &Address, value: u8) {
        match address {
            &Self::DIV => self.div = 0,
            &Self::TIMA => self.tima = value,
            &Self::TMA => self.tma = value,
            &Self::TAC => {
                self.tac = (self.tac & !(Self::TAC_ENABLE_BIT | Self::TAC_FREQ_BITS))
                    | (value & (Self::TAC_ENABLE_BIT | Self::TAC_FREQ_BITS))
            }
            _ => unreachable!(),
        }
    }

    pub(super) const fn read(&self, address: &Address) -> u8 {
        match address {
            // return most significant bit
            &Self::DIV => self.div(),
            &Self::TIMA => self.tima,
            &Self::TMA => self.tma,
            &Self::TAC => self.tac(),
            _ => unreachable!(),
        }
    }

    pub fn cycle(&mut self, int: &mut u8, cycles: &Cycles) {
        let ticks = cycles.t() as u16;

        let old_div = self.div;

        self.div = self.div.wrapping_add(ticks);

        if self.overflow {
            self.overflow = false;
            *int |= Self::INTERRUPT_BIT;
            self.tima = self.tma;
        }

        let freq = self.step();
        let increase_tima = (old_div.wrapping_add(ticks) / freq).wrapping_sub(old_div / freq) as u8;

        // If bit 2 of TAC is set to 0 then the timer is disabled
        if increase_tima != 0 && self.enabled() {
            if self.tima == 0xFF {
                self.tima = 0;
                self.overflow = true;
            } else {
                self.tima = self.tima.wrapping_add(increase_tima);
            }
        }
    }

    const fn step(&self) -> u16 {
        match self.tac & Self::TAC_FREQ_BITS {
            0b00 => 128,
            0b01 => 2,
            0b10 => 8,
            0b11 => 32,
            _ => unreachable!(),
        }
    }

    const fn enabled(&self) -> bool {
        self.tac & Self::TAC_ENABLE_BIT != 0
    }

    pub const fn div(&self) -> u8 {
        (self.div >> 8) as u8
    }

    pub const fn tima(&self) -> u8 {
        self.tima
    }

    pub const fn tma(&self) -> u8 {
        self.tma
    }

    pub const fn tac(&self) -> u8 {
        self.tac & 0b111
    }
}
