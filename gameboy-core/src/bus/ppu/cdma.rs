use std::f32::consts::E;

use crate::bus::Bus;
use crate::bus::ppu::Ppu;
use crate::cpu::{CycleResult, ExecutionType};
use crate::util::Address;
use crate::{Cartridge, Cycles, MemoryError, Width};

#[derive(Default, Clone, Copy, Debug)]
pub struct Cdma {
    hdma5: u8,
    source: Width,
    destination: Width,
    index: Option<Width>,
}

impl Cdma {
    pub const ADDRESS_HDMA1: Address = Address::new(0xFF51); // CGB only, HDMA source high
    pub const ADDRESS_HDMA2: Address = Address::new(0xFF52); // CGB only, HDMA source low
    pub const ADDRESS_HDMA3: Address = Address::new(0xFF53); // CGB only, HDMA destination high
    pub const ADDRESS_HDMA4: Address = Address::new(0xFF54); // CGB only, HDMA destination low
    pub const ADDRESS_HDMA5: Address = Address::new(0xFF55); // CGB only, HDMA length/mode

    pub const fn is_active(&self, ppu: &Ppu) -> bool {
        self.index.is_some() && (!self.hblank_mode() || ppu.mode() == Ppu::HBLANK)
    }

    pub(crate) const fn read(&self, address: &Address) -> Result<u8, MemoryError> {
        match address {
            &Self::ADDRESS_HDMA5 => Ok(self.hdma5 | if self.index.is_some() { 0 } else { 0x80 }),
            _ => Err(MemoryError::Read("CGB DMA", address.index())),
        }
    }

    pub(crate) const fn write(&mut self, address: &Address, value: u8) {
        match address {
            &Self::ADDRESS_HDMA1 => self.source = self.source & 0x00FF | ((value as u16) << 8),
            &Self::ADDRESS_HDMA2 => self.source = self.source & 0xFF00 | (value as u16 & 0xF0),
            &Self::ADDRESS_HDMA3 => {
                self.destination = self.destination & 0x00FF | ((value as u16 & 0xF) << 8)
            }
            &Self::ADDRESS_HDMA4 => {
                self.destination = self.destination & 0xFF00 | (value as u16 & 0xF0)
            }
            &Self::ADDRESS_HDMA5 => match self.index {
                Some(..) => {
                    self.hdma5 = value & 0x7F;
                    self.index = None;
                }
                None => {
                    self.hdma5 = value & 0x7F;
                    self.index = Some(0);
                }
            },
            _ => unreachable!(),
        }
    }

    const fn length(&self) -> Width {
        ((self.hdma5 & 0x7F) + 1) as Width * 16
    }

    const fn hblank_mode(&self) -> bool {
        self.hdma5 & (1 << 7) != 0
    }

    pub(crate) fn cycle(
        mut self,
        result: &CycleResult,
        cart: &dyn Cartridge,
        bus: &mut Bus,
    ) -> Result<Cycles, MemoryError> {
        if matches!(result.kind, ExecutionType::Halt | ExecutionType::Stop) {
            return Ok(Cycles(0));
        }
        let length = self.length();
        let hblank_mode = self.hblank_mode();
        let mut vram_cycles: usize = 0;
        if let Some(index) = self.index.as_mut() {
            vram_cycles = if hblank_mode {
                0x10
            } else {
                result.cycles.t() / 2
            } as usize;
            for _ in 0..vram_cycles {
                let value = bus
                    .read::<true>(cart, Address::new(self.source + *index))
                    .unwrap_or(0xFF);
                bus.ppu
                    .vram
                    .write_offset(Address::new(self.destination + *index), value)?;
                *index += 1;
                if *index >= length {
                    self.index = None;
                    break;
                }
            }
        }
        bus.cdma = self;
        Ok(Cycles(vram_cycles * 4))
    }
}
