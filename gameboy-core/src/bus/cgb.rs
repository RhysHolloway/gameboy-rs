use crate::util::{Address, MemoryError};

#[derive(Default)]
pub struct Cgb {
    key1: u8,
}

impl Cgb {

    const KEY1_START: Address = Address::new(0xFF4D);

    pub const fn read_mapped(&self, address: &Address) -> Result<u8, MemoryError> {
        match address {
            &Self::KEY1_START => Ok(self.key1),
            _ => Err(MemoryError::Read("CGB", *address)),
        }
    }

    pub const fn write_mapped(&mut self, address: &Address, value: u8) -> Result<(), MemoryError> {
        match address {
             &Self::KEY1_START => self.key1 = value & 1,
            _ => return Err(MemoryError::Write("CGB", *address)),
        }
        Ok(())
    }

    pub fn speed(&self) -> u8 {
        (self.key1 & 1) + 1
    }

    pub fn disarm(&mut self) -> bool {
        if self.key1 & 1 == 1 {
            self.key1 &= !1;
            true
        } else {
            false
        }

    }

}