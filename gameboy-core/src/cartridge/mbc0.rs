use crate::Address;


pub struct MBC0 {
    rom: Vec<u8>,
}

impl MBC0 {
    pub fn from_vec(data: Vec<u8>) -> MBC0 {
        MBC0 { rom: data }
    }
}

impl super::Cartridge for MBC0 {
    fn rom(&self) -> &[u8] {
        self.rom.as_slice()
    }

    fn ram_mut(&mut self) -> &mut [u8] {
        &mut []
    }

    fn read(&self, address: Address) -> Result<u8, crate::MemoryError> {
        match address.value() {
            0x0000..=0x7FFF => Ok(self.readrom(address.value())),
            0xA000..=0xBFFF => Ok(self.readram(address.value())),
            _ => Err(crate::MemoryError::Read("MBC0", address.index())),
        }
    }

    fn write(&mut self, address: Address, value: u8) -> Result<(), crate::MemoryError> {
        match address.value() {
            0x0000..=0x7FFF => self.writerom(address.value() as u16, value),
            0xA000..=0xBFFF => self.writeram(address.value() as u16, value),
            _ => return Err(crate::MemoryError::Write("MBC0", address.index())),
        }
        Ok(())
    }

    fn new(data: impl AsRef<[u8]>) -> Self
    where
        Self: Sized,
    {
        Self::from_vec(data.as_ref().to_vec())
    }
    
    fn ram(&self) -> &[u8] {
        &[]
    }
}

impl MBC0 {
    fn readrom(&self, a: u16) -> u8 {
        self.rom[a as usize]
    }
    fn readram(&self, _a: u16) -> u8 {
        0
    }
    fn writerom(&mut self, _a: u16, _v: u8) {
        ()
    }
    fn writeram(&mut self, _a: u16, _v: u8) {
        ()
    }

    fn is_battery_backed(&self) -> bool {
        false
    }
    fn check_and_reset_ram_updated(&mut self) -> bool {
        false
    }
}
