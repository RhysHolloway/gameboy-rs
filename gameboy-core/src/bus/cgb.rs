use crate::{Cartridge, Width};
use crate::util::{Address, MemoryError};

#[derive(Default)]
pub struct Cgb {
    enabled: bool,
    key1: u8,
    cdma: super::ppu::Cdma,
}

impl Cgb {
    const ADDRESS_KEY0: Width = 0xFF4C;
    const ADDRESS_KEY1: Width = 0xFF4D;

    pub fn set_enabled(&mut self, cart: &dyn Cartridge) {
        self.enabled = cart.color();
    }

    pub const fn read_mapped(&self, address: &Address) -> Result<u8, MemoryError> {
        if !self.enabled {
            return Ok(u8::MAX);
        }
        Ok(match address.value() {
            Self::ADDRESS_KEY0 => 0,
            Self::ADDRESS_KEY1 => self.key1,
            0xFF51..=0xFF55 => match self.cdma.read(address) {
                Ok(value) => value,
                Err(err) => return Err(err),
            },
            _ => unreachable!(),
        })
    }

    pub const fn write_mapped(&mut self, address: &Address, value: u8) -> Result<(), MemoryError> {
        if !self.enabled {
            return Ok(());
        }
        match address.value() {
            Self::ADDRESS_KEY1 => self.key1 = value & 1,
            0xFF51..=0xFF55 => self.cdma.write(&address, value),
            _ => return Err(MemoryError::Write("CGB", address.index())),
        }
        Ok(())
    }

    pub fn double_speed(&self) -> bool {
        self.key1 & 0b10000000 != 0
    }

    pub fn disarm(&mut self) -> bool {
        if self.key1 & 1 == 1 {
            self.key1 = !1;
            true
        } else {
            false
        }
    }
    
    pub(crate) const fn enabled(&self) -> bool {
        self.enabled
    }
}
