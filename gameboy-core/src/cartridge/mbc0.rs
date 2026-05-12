use crate::Address;

pub struct MBC0 {
    rom: Vec<u8>,
}

impl super::Cartridge for MBC0 {
    fn new(data: impl AsRef<[u8]>) -> Self
    where
        Self: Sized,
    {
        Self {
            rom: data.as_ref().to_vec(),
        }
    }

    fn rom(&self) -> &[u8] {
        self.rom.as_slice()
    }

    fn ram_mut(&mut self) -> &mut [u8] {
        &mut []
    }

    fn ram(&self) -> &[u8] {
        &[]
    }

    fn read(&self, address: Address) -> u8 {
        match address.value() {
            0x0000..=0x7FFF => self.rom[address.index()],
            0xA000..=0xBFFF => 0,
            _ => unreachable!(),
        }
    }

    fn write(&mut self, _address: Address, _value: u8) {}
}
