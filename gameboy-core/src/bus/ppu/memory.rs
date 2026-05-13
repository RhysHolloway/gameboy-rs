use crate::bus::ppu::PpuRegisters;
use crate::{Address, Memory};

pub type Voam = Memory<0xA0>;

#[derive(Default, Clone)]
pub struct Vram {
    pub(super) memory: Memory<{ Self::VRAM_BANK_SIZE * 2 }>,
    pub(super) bank: bool,
}

impl Vram {
    pub const fn read(&self, index: usize) -> u8 {
        self.memory.read(index)
    }

    pub const fn map<const DMA: bool>(
        &self,
        regs: &PpuRegisters,
        address: &Address,
    ) -> Option<usize> {
        if DMA || regs.mode() != PpuRegisters::TRANSFER {
            Some(self.bank(address.index() - 0x8000))
        } else {
            None
        }
    }

    pub const fn bank(&self, offset: usize) -> usize {
        offset + (self.bank as usize * Vram::VRAM_BANK_SIZE)
    }

    pub const fn write(&mut self, index: usize, value: u8) {
        self.memory.write(index, value);
    }
}

impl Vram {
    pub const VRAM_BANK_SIZE: usize = 0x2000;
}
