use crate::bus::Bus;
use crate::bus::ppu::Ppu;
use crate::cpu::{CycleResult, ExecutionType};
use crate::util::Address;
use crate::{Cartridge, Cycles, MemoryError, Width};

#[derive(Default, Clone, Copy, Debug)]
pub struct Cdma {
    hdma12: [u8; 2],
    hdma34: [u8; 2],
    hdma5: u8,
    transfer: Option<CdmaState>,
}

#[derive(Clone, Copy, Debug)]
struct CdmaState {
    source: Width,
    destination: Width,
    index: Width,
}

impl Cdma {
    pub const ADDRESS_HDMA1: Address = Address::new(0xFF51); // CGB only, HDMA source high
    pub const ADDRESS_HDMA2: Address = Address::new(0xFF52); // CGB only, HDMA source low
    pub const ADDRESS_HDMA3: Address = Address::new(0xFF53); // CGB only, HDMA destination high
    pub const ADDRESS_HDMA4: Address = Address::new(0xFF54); // CGB only, HDMA destination low
    pub const ADDRESS_HDMA5: Address = Address::new(0xFF55); // CGB only, HDMA length/mode

    pub const fn is_active(&self, ppu: &Ppu) -> bool {
        self.transfer.is_some() && (!self.hblank_mode() || ppu.mode() == Ppu::HBLANK)
    }

    pub(crate) const fn read(&self, address: &Address) -> Result<u8, MemoryError> {
        match address {
            &Self::ADDRESS_HDMA5 => Ok(self.hdma5 | if self.transfer.is_some() { 0x80 } else { 0 }),
            _ => Err(MemoryError::Read("CGB DMA", address.index())),
        }
    }

    pub(crate) const fn write(&mut self, address: &Address, value: u8) {
        match address {
            &Self::ADDRESS_HDMA1 => self.hdma12[0] = value,
            &Self::ADDRESS_HDMA2 => self.hdma12[1] = value & 0xF0,
            &Self::ADDRESS_HDMA3 => self.hdma34[0] = value & 0x1F,
            &Self::ADDRESS_HDMA4 => self.hdma34[1] = value & 0xF0,
            &Self::ADDRESS_HDMA5 => match self.transfer.is_some() && self.hblank_mode() {
                true => {
                    if value & 0x80 == 0 {
                        self.transfer = None;
                    }
                }
                false => {
                    self.hdma5 = value & 0x7F;
                    self.transfer = Some(CdmaState {
                        source: u16::from_be_bytes(self.hdma12),
                        destination: u16::from_be_bytes(self.hdma34),
                        index: 0,
                    });
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
        if let Some(CdmaState {
            source,
            destination,
            index,
        }) = self.transfer.as_mut()
        {
            vram_cycles = if hblank_mode {
                0x10
            } else {
                result.cycles.t() / 2
            } as usize;
            for _ in 0..vram_cycles {
                let value = bus
                    .read::<true>(cart, Address::new(*source + *index))
                    .unwrap_or(0xFF);
                bus.ppu
                    .vram
                    .write((*destination + *index) as usize, value)?;
                *index += 1;
                if *index >= length {
                    self.transfer = None;
                    break;
                }
            }
        }
        bus.cdma = self;
        Ok(Cycles(vram_cycles * 4))
    }
}
