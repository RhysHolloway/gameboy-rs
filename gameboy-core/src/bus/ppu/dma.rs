use crate::bus::Bus;
use crate::util::Address;
use crate::{Cartridge, Cycles, Width};

#[derive(Clone, Debug)]
pub struct Dma {
    source: Address,
    clock: Width,
}

impl Default for Dma {
    fn default() -> Self {
        Self {
            source: Address::new(0),
            clock: Self::END,
        }
    }
}

impl Dma {
    const LENGTH: Width = 160;
    // t cycles * length of data
    const END: Width = Self::LENGTH * 4;

    pub const fn is_active(&self) -> bool {
        self.clock < Self::END
    }

    pub const fn read(&self) -> u8 {
        (self.source.value() >> 8) as u8
    }

    pub(crate) const fn write(&mut self, value: u8) {
        self.source = Address::new((value as Width) << 8);
        self.clock = 0;
    }
}

impl Bus {
    pub(crate) fn cycle_dma(&mut self, cycles: &Cycles, cart: &dyn Cartridge) {
        if !self.dma.is_active() {
            return;
        }
        for _ in 0..cycles.t() {
            if self.dma.clock % 4 == 0 {
                let index = self.dma.clock / 4;
                let value = self.read::<true>(cart, self.dma.source + index);
                self.ppu.voam.write(index as usize, value);
            }
            self.dma.clock += 1;
            if !self.dma.is_active() {
                break;
            }
        }
    }
}
