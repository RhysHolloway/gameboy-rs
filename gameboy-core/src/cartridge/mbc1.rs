use crate::Address;
use crate::cartridge::{ram_banks, rom_banks};

pub struct MBC1 {
    rom: Vec<u8>,
    ram: Box<[u8]>,
    ram_enabled: bool,
    bank_mode: bool,
    bank_bits: u8,
    rom_bank_lower: u8,
    rom_banks: u8,
    ram_banks: u8,
}

impl MBC1 {
    const fn rom_bank(&self) -> u8 {
        match self.bank_mode {
            false => self.rom_bank_lower,
            true => self.rom_bank_lower | (self.bank_bits << 5),
        }
    }

    const fn ram_bank(&self) -> Option<u8> {
        if self.ram_enabled {
            if self.ram_banks == 4 && !self.bank_mode {
                return Some(self.bank_bits);
            } else if self.ram_banks > 0 {
                return Some(0);
            }
        }
        None
    }
}

impl super::Cartridge for MBC1 {
    fn new(data: impl AsRef<[u8]>) -> Self
    where
        Self: Sized,
    {
        let data = data.as_ref();

        let ram_banks = match data[0x147] {
            0x02 | 0x03 => ram_banks(&data),
            _ => 0,
        };

        Self {
            rom: data.to_vec(),
            ram: unsafe { Box::new_zeroed_slice(ram_banks as usize * 0x2000).assume_init() },
            ram_enabled: false,
            bank_mode: false,
            bank_bits: 0,
            rom_bank_lower: 1,
            rom_banks: rom_banks(&data) as u8,
            ram_banks,
        }
    }

    fn rom(&self) -> &[u8] {
        self.rom.as_slice()
    }

    fn ram(&self) -> &[u8] {
        &self.ram
    }

    fn ram_mut(&mut self) -> &mut [u8] {
        &mut self.ram
    }

    fn read(&self, address: Address) -> u8 {
        match address.value() {
            0x0000..=0x3FFF => self.rom[address.index()],
            0x4000..=0x7FFF => {
                self.rom[self.rom_bank() as usize * 0x4000 | (address.index() & 0x3FFF)]
            }
            0xA000..=0xBFFF => self
                .ram_bank()
                .map(|bank| self.ram[(bank as usize * 0x2000) | (address.index() & 0x1FFF)])
                .unwrap_or(0xFF),
            _ => unreachable!(),
        }
    }

    fn write(&mut self, address: Address, value: u8) {
        match address.value() {
            0x0000..=0x1FFF => self.ram_enabled = value & 0xF == 0xA,
            0x2000..=0x3FFF => {
                self.rom_bank_lower = 1.max(value & 0x1F) % self.rom_banks;
            }
            0x4000..=0x5FFF => {
                if self.ram_banks == 4 || self.rom_banks >= 0x40 {
                    self.bank_bits = value & 0x03;
                }
            }
            0x6000..=0x7FFF => {
                if self.ram_banks > 1 || self.rom_banks > 0x20 {
                    self.bank_mode = value & 0x01 == 1;
                }
            }
            0xA000..=0xBFFF => {
                if let Some(bank) = self.ram_bank() {
                    self.ram[(bank as usize * 0x2000) | (address.index() & 0x1FFF)] = value;
                }
            }
            _ => unreachable!(),
        }
    }
}
