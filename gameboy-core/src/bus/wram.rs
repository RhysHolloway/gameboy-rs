use crate::util::{Address, MemoryError};

pub struct Wram {
    data: Vec<u8>,
    bank: u8,
}

impl Default for Wram {
    fn default() -> Self {
        Self {
            data: vec![0; Self::BANK_SIZE * 8],
            bank: 1,
        }
    }
}

impl Wram {
    pub const WRAM_START: Address = Address::new(0xC000);
    pub const WRAM_END: Address = Address::new(0xDFFF);

    pub const BANK_SIZE: usize = 0x1000;

    pub const BANK_ADDRESS: Address = Address::new(0xFF70);

    pub const fn write_bank(&mut self, value: u8) {
        self.bank = if value == 0 { 1 } else { value & 0b111 };
    }

    pub const fn read_bank(&self) -> u8 {
        self.bank
    }

    pub fn read(&self, address: Address) -> Result<u8, MemoryError> {
        let index = address.index();
        self.data
            .get(self.map(index))
            .copied()
            .ok_or_else(|| MemoryError::Read("Work RAM", index))
    }

    pub fn write(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        let index = self.map(address.index());
        *self
            .data
            .get_mut(index)
            .ok_or_else(|| MemoryError::Write("Work RAM", index))? = value;
        Ok(())
    }

    const fn map(&self, offset: usize) -> usize {
        match offset {
            0x0000..=0x0FFF => offset,
            0x1000..=0x1FFF => offset + (self.bank - 1) as usize * Self::BANK_SIZE,
            _ => usize::MAX,
        }
    }
}
