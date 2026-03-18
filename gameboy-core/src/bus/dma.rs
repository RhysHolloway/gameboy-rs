use crate::bus::Bus;
use crate::{Cartridge, Cycles};
use crate::util::{Address, MemoryError};

#[derive(Clone, Copy, Debug, Default)]
pub struct Dma {
    active: bool,
    value: u8,
    index: u16,
    cycle_counter: u8,
}

impl Dma {

    pub const fn is_active(&self) -> bool {
        self.active
    }

    pub(super) const fn read(&self) -> u8 {
        self.value
    }

    pub(super) const fn write(&mut self, value: u8) {
        self.value = value;
        self.index = 0;
        self.cycle_counter = 0;
        self.active = true;
    }

    pub(super) fn cycle<D: AsRef<[u8]>>(cycles: &Cycles, rom: &Cartridge<D>, bus: &mut Bus) -> Result<(), MemoryError> {
        let mut this = std::mem::take(&mut bus.dma);
        if !this.active {
            return Ok(());
        }
        for _ in 0..cycles.t() {
            this.cycle_counter += 1;
            if this.cycle_counter >= 4 {
                this.cycle_counter = 0;
                let address = Address::new(((this.value as u16) << 8) | this.index);
                let value = bus.read_dma(rom, address).unwrap_or(0xFF);
                bus.ppu.voam.write_offset(Address::new(this.index), value)?;
                this.index += 1;
                if this.index >= 160 {
                    this.active = false;
                    break;
                }
            }
        }
        bus.dma = this;
        Ok(())
    }
}