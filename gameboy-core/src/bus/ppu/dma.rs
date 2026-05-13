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
            clock: Self::LENGTH,
        }
    }
}

impl Dma {
    const LENGTH: Width = 160;

    pub const fn is_active(&self) -> bool {
        self.clock < Self::LENGTH
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
        let start = self.dma.clock;
        self.dma.clock += cycles.t() as u16;
        for index in start..self.dma.clock.min(Dma::LENGTH) {
            let value = self.read::<true>(cart, self.dma.source + index);
            self.ppu.voam.write(index as usize, value);
        }
    }
}
