use crate::util::{Address, Memory, MemoryError};

pub struct Wram {
    data: Memory<{ Self::SIZE }>,
    bank: u8,
}

impl Default for Wram {
    fn default() -> Self {
        Self {
            data: Memory::new("Work RAM"),
            bank: 1,
        }
    }
}

impl Wram {
    pub const fn read_offset(&self, address: Address) -> Result<u8, MemoryError> {
        self.data.read_offset(address)
    }

    pub const fn write_offset(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        self.data.write_offset(address, value)
    }

    const fn map(&self, address: Address) -> Address {
        let offset = address.sub(Self::WRAM_START);
        Address::from_index(match offset {
            0x0000..=0x0FFF => offset,
            0x1000..=0x1FFF => offset + (self.bank - 1) as usize * 0x4000,
            _ => usize::MAX,
        })
    }

    pub const fn read_mapped(&self, address: Address) -> Result<u8, MemoryError> {
        match self.read_offset(self.map(address)) {
            Ok(value) => Ok(value),
            Err(e) => Err(MemoryError::Read(e.location, address)),
        }
    }

    pub const fn write_mapped(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        match self.write_offset(self.map(address), value) {
            Ok(()) => Ok(()),
            Err(e) => Err(MemoryError::Write(e.location, address)),
        }
    }
}

impl Wram {
    pub const WRAM_START: usize = 0xC000;
    pub const WRAM_END: usize = 0xDFFF;

    pub const SIZE: usize = Self::WRAM_END - Self::WRAM_START + 1;
}
