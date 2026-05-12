use crate::Memory;

pub struct Wram {
    data: Memory<{ Self::BANK_SIZE * 8 }>,
    bank: u8,
}

impl Default for Wram {
    fn default() -> Self {
        Self {
            data: Memory::default(),
            bank: 1,
        }
    }
}

impl Wram {
    pub const BANK_SIZE: usize = 0x1000;

    pub const fn write_bank(&mut self, value: u8) {
        self.bank = (value & 0b111).saturating_sub(1);
    }

    pub const fn read_bank(&self) -> u8 {
        self.bank + 1
    }

    pub const fn read(&self, offset: usize) -> u8 {
        self.data.read(self.map(offset))
    }

    pub const fn write(&mut self, offset: usize, value: u8) {
        self.data.write(self.map(offset), value);
    }

    const fn map(&self, offset: usize) -> usize {
        match offset {
            0x0000..=0x0FFF => offset,
            0x1000..=0x1FFF => offset + self.bank as usize * Self::BANK_SIZE,
            _ => unreachable!(),
        }
    }
}
