use super::{ram_banks, rom_banks};
use crate::{Address, Cartridge};

pub struct MBC5 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    ram_on: bool,
    rumble: bool,
    rombank: usize,
    rambank: usize,
    rombanks: usize,
    rambanks: usize,
}

impl MBC5 {
    pub fn from_vec(data: Vec<u8>) -> Self {
        let rambanks = match data[0x147] {
            0x1A | 0x1B | 0x1D | 0x1E => ram_banks(&data) as usize,
            _ => 0,
        };

        let rumble = matches!(data[0x147], 0x1C..=0x1E);

        Self {
            rombanks: rom_banks(&data) as usize,
            rom: data,
            ram: vec![0; rambanks * 0x2000],
            ram_on: false,
            rumble,
            rombank: 1,
            rambank: 0,
            rambanks,
        }
    }

    fn readrom(&self, address: u16) -> u8 {
        let bank = if address < 0x4000 {
            0
        } else {
            self.rombank % self.rombanks
        };
        let index = bank * 0x4000 + ((address as usize) & 0x3FFF);
        self.rom[index]
    }

    fn readram(&self, address: u16) -> u8 {
        if !self.ram_on || self.rambank >= self.rambanks {
            return 0xFF;
        }
        self.ram[self.rambank * 0x2000 + ((address as usize) & 0x1FFF)]
    }

    fn writerom(&mut self, address: u16, value: u8) {
        match address {
            0x0000..=0x1FFF => self.ram_on = value & 0x0F == 0x0A,
            0x2000..=0x2FFF => {
                self.rombank = (self.rombank & 0x100) | value as usize;
            }
            0x3000..=0x3FFF => {
                self.rombank = (self.rombank & 0x0FF) | (((value & 1) as usize) << 8);
            }
            0x4000..=0x5FFF => {
                let mask = if self.rumble { 0x07 } else { 0x0F };
                self.rambank = (value & mask) as usize;
            }
            0x6000..=0x7FFF => (),
            _ => unreachable!(),
        }
    }

    fn writeram(&mut self, address: u16, value: u8) {
        if !self.ram_on || self.rambank >= self.rambanks {
            return;
        }

        let index = self.rambank * 0x2000 + ((address as usize) & 0x1FFF);
        if let Some(byte) = self.ram.get_mut(index) {
            *byte = value;
        }
    }
}

impl Cartridge for MBC5 {
    fn new(data: impl AsRef<[u8]>) -> Self
    where
        Self: Sized,
    {
        Self::from_vec(data.as_ref().to_vec())
    }

    fn read(&self, address: Address) -> u8 {
        match address.value() {
            0x0000..=0x7FFF => self.readrom(address.value()),
            0xA000..=0xBFFF => self.readram(address.value()),
            _ => unreachable!(),
        }
    }

    fn write(&mut self, address: Address, value: u8) {
        match address.value() {
            0x0000..=0x7FFF => self.writerom(address.value(), value),
            0xA000..=0xBFFF => self.writeram(address.value(), value),
            _ => unreachable!(),
        }
    }

    fn rom(&self) -> &[u8] {
        &self.rom
    }

    fn ram(&self) -> &[u8] {
        &self.ram
    }

    fn ram_mut(&mut self) -> &mut [u8] {
        &mut self.ram
    }
}
