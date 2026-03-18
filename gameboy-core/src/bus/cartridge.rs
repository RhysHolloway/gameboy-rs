use crate::Cartridge;
use crate::util::{Address, MemoryError};

use super::Bank;

pub struct CartridgeBus {
    rom_bank: Bank,
    ram_bank: Bank,
    ram_enabled: bool,
    select: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum CartridgeError {
    Rom(MemoryError, Bank),
    Ram(MemoryError, Bank, bool),
}

impl core::fmt::Display for CartridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CartridgeError::Rom(error, bank) => write!(f, "{error} in cartridge ROM bank {bank:02X}"),
            CartridgeError::Ram(error, bank, enabled) => {
                match enabled {
                    true => write!(f, "{error} in cartridge RAM bank {bank:02X}"),
                    false => write!(f, "{error} in cartridge RAM as is not enabled"),
                }
            }
        }
    }
}

impl Default for CartridgeBus {
    fn default() -> Self {
        Self::new()
    }
}

impl CartridgeBus {

    const ROM_BANK_MIN: Address = Address::new(0x4000);
    const RAM_START: Address = Address::new(0xA000);
    const RAM_SIZE: usize = 0x2000;

    pub const fn new() -> Self {
        Self { rom_bank: 0, ram_bank: 0, ram_enabled: false, select: false }
    }

    pub(super) fn read_rom<D: AsRef<[u8]>>(&self, rom: &Cartridge<D>, address: Address) -> Result<u8, CartridgeError> {
        match address.value() {
            0x0000..=0x3FFF => rom.read(address.index()),
            0x4000..=0x7FFF => rom.read(address.index() + self.rom_bank.saturating_sub(1) as usize * 0x4000),
            0x8000.. => unreachable!(),
        }.ok_or(CartridgeError::Rom(MemoryError::Read("Cartridge ROM", address), self.rom_bank))
    }

    pub(super) const fn write_rom(&mut self, address: Address, value: u8) -> Result<(), CartridgeError> {
        match address.value() {
            0x0000..=0x1FFF => {
                self.ram_enabled = value == 0x0A;
            }
            0x2000..=0x3FFF => {
                self.rom_bank = value & 0x7F;
                if self.rom_bank == 0 {
                    self.rom_bank = 1;
                }
            }
            0x4000..=0x5FFF => {
                if self.select {
                    self.ram_bank = value;
                    // printf("SET RAM BANK TO 0x%02X\n", cart->ram_bank);
                } else {
                    self.rom_bank = (self.rom_bank & 31) | (value << 5);
                    // printf("SET ROM BANK (UPPER) TO 0x%02X\n", cart->rom_bank);
                }
            }
            0x6000..=0x7FFF => {
                if value <= 1 {
                    self.select = value == 1;
                }
            }
            0x8000.. => return Err(CartridgeError::Rom(MemoryError::Write("Cartridge ROM", address), self.rom_bank)),
        }
        Ok(())
    }

    const fn ram_address(&self, address: Address) -> usize {
        address.index() - Self::RAM_START.index() + self.ram_bank as usize * 0x2000
    }

    pub(super) fn read_ram<D: AsRef<[u8]>>(&self, cart: &Cartridge<D>, address: Address) -> Result<u8, CartridgeError> {
        match cart.ram.get(self.ram_address(address)) {
            Some(value) => Ok(*value),
            None => Err(CartridgeError::Ram(MemoryError::Read("Cartridge RAM", address), self.ram_bank, self.ram_enabled))
        }
    }

    pub(super) fn write_ram<D: AsRef<[u8]>>(&mut self, cart: &mut Cartridge<D>, address: Address, value: u8) -> Result<(), CartridgeError> {
        match cart.ram.get_mut(self.ram_address(address)) {
            Some(ptr) => Ok(*ptr = value),
            None => Err(CartridgeError::Ram(MemoryError::Write("Cartridge RAM", address), self.ram_bank, self.ram_enabled))
        }
    }

    pub const fn rom_bank(&self) -> u8 {
        self.rom_bank
    }
}
