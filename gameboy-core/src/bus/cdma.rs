use crate::bus::Bus;
use crate::bus::ppu::PpuRegisters;
use crate::cpu::{CycleResult, ExecutionType};
use crate::util::Address;
use crate::{Cartridge, Cycles, Width};

#[derive(Clone, Debug)]
pub struct Cdma {
    source: Width,
    destination: Width,
    hdma5: u8,
    transfer: Option<Transfer>,
}

#[derive(Clone, Debug)]
struct Transfer {
    source: Width,
    destination: Width,
    index: Width,
    length: Width,
    hdma: bool,
}

impl Transfer {
    const BLOCK_SIZE: Width = 0x10;

    fn remaining(&self) -> u8 {
        ((self.length - self.index / Self::BLOCK_SIZE) as u8).wrapping_sub(1)
    }
}

impl Cdma {
    pub const ADDRESS_HDMA1: Address = Address::new(0xFF51); // CGB only, HDMA source high
    pub const ADDRESS_HDMA2: Address = Address::new(0xFF52); // CGB only, HDMA source low
    pub const ADDRESS_HDMA3: Address = Address::new(0xFF53); // CGB only, HDMA destination high
    pub const ADDRESS_HDMA4: Address = Address::new(0xFF54); // CGB only, HDMA destination low
    pub const ADDRESS_HDMA5: Address = Address::new(0xFF55); // CGB only, HDMA length/mode

    pub(crate) const fn read(&self, address: &Address) -> u8 {
        match address {
            &Self::ADDRESS_HDMA1
            | &Self::ADDRESS_HDMA2
            | &Self::ADDRESS_HDMA3
            | &Self::ADDRESS_HDMA4 => u8::MAX,
            &Self::ADDRESS_HDMA5 => self.hdma5,
            _ => unreachable!(),
        }
    }

    pub(crate) fn write(&mut self, address: &Address, value: u8) {
        match address {
            &Self::ADDRESS_HDMA1 => self.source = (self.source & 0x00FF) | ((value as Width) << 8),
            &Self::ADDRESS_HDMA2 => self.source = (self.source & 0xFF00) | (value as Width),
            &Self::ADDRESS_HDMA3 => {
                self.destination = (self.destination & 0x00FF) | ((value as Width) << 8)
            }
            &Self::ADDRESS_HDMA4 => {
                self.destination = (self.destination & 0xFF00) | (value as Width)
            }
            &Self::ADDRESS_HDMA5 => {
                if let Some(transfer) = self.transfer.as_ref() {
                    if transfer.hdma {
                        if value & 0x80 == 0 {
                            self.hdma5 = 0x80 | transfer.remaining();
                            self.transfer = None;
                        }
                        return;
                    }
                }

                self.hdma5 = value & 0x7F;
                self.transfer = Some(Transfer {
                    source: self.source & 0xFFF0,
                    destination: (self.destination & 0x1FF0) | 0x8000,
                    length: ((value & 0x7F) + 1) as Width * Transfer::BLOCK_SIZE,
                    hdma: value & 0x80 != 0,
                    index: 0,
                });
            }
            _ => unreachable!(),
        }
    }
}

impl Default for Cdma {
    fn default() -> Self {
        Self {
            source: 0,
            destination: 0,
            transfer: None,
            hdma5: u8::MAX,
        }
    }
}

impl Bus {
    pub(crate) fn cdma_cycle(&mut self, result: &mut CycleResult, cart: &dyn Cartridge) -> Cycles {
        if let Some(transfer) = self
            .cdma
            .transfer
            .as_mut()
            .filter(|transfer| !transfer.hdma || self.ppu.mode() == PpuRegisters::HBLANK)
            && !matches!(result.kind, ExecutionType::Halt | ExecutionType::Stop)
        {
            let cycles = match transfer.hdma {
                true => Cycles(32), // 0x10 blocks take 32 t-cycles to transfer
                false => result.cycles,
            };

            let source = transfer.source;
            let destination = transfer.destination - 0x8000;

            let start = transfer.index;
            let end =
                transfer.index + (cycles.t() as u16).min(transfer.length - transfer.index) as Width;

            if end >= transfer.length {
                self.cdma.transfer = None;
                self.cdma.hdma5 = u8::MAX;
            } else {
                transfer.index = end;
                self.cdma.hdma5 = transfer.remaining();
            }

            for i in start..end {
                let value = self.read::<true>(cart, Address::new(source + i));
                self.ppu
                    .vram
                    .write(self.ppu.vram.bank((destination + i) as usize), value);
            }

            cycles
        } else {
            Cycles(0)
        }
    }
}
