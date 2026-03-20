use crate::bus::Bus;
use crate::util::Address;
use crate::{Cartridge, Cycles, MemoryError, Width};

#[derive(Clone, Copy, Debug)]
pub struct Dma {
    source: Address,
    index: Width,
}

impl Default for Dma {
    fn default() -> Self {
        Self {
            source: Address::new(0),
            index: 160,
        }
    }
}

impl Dma {
    pub const fn is_active(&self) -> bool {
        self.index < 160
    }

    pub(crate) const fn read(&self) -> u8 {
        (self.source.value() >> 8) as u8
    }

    pub(crate) const fn write(&mut self, value: u8) {
        self.source = Address::new((value as Width) << 8);
        self.index = 0;
    }

    pub(crate) fn cycle(
        mut self,
        cycles: &Cycles,
        cart: &dyn Cartridge,
        bus: &mut Bus,
    ) -> Result<(), MemoryError> {
        if self.is_active() {
            for _ in 0..cycles.m() {
                let value = bus
                    .read::<true>(cart, self.source + self.index)
                    .unwrap_or(0xFF);
                bus.ppu.voam.write_offset(Address::new(self.index), value)?;
                self.index += 1;
                if self.index >= 160 {
                    break;
                }
            }
        }
        bus.dma = self;
        Ok(())
    }
}
