mod mbc3;
mod mbc1;
mod mbc0;
mod mbc_funcs;

use crate::{Address, MemoryError};

#[derive(Debug)]
pub enum CartridgeError {
    NoHeader,
    NotSupported(u8),
}

impl std::fmt::Display for CartridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoHeader => write!(f, "Cartridge has an invalid header!"),
            Self::NotSupported(t) => write!(f, "Cartridge type 0x{t:02X} is not supported!"),
        }
    }
}

// pub struct BasicCartridge<D: AsRef<[u8]>> {
//     data: D,
//     ram: Vec<u8>,
//     rom_bank: u8,
//     ram_bank: u8,
//     ram_enabled: bool,
// }

// impl<D: AsRef<[u8]>> BasicCartridge<D> {
//     pub fn new(data: D) -> Option<Self> {
//         if data.as_ref().len() < 0x150 {
//             return None;
//         }
//         Some(Self { data, ram: Vec::new(), rom_bank: 1, ram_bank: 0, ram_enabled: false })
//     }
// }

// impl<D: AsRef<[u8]>> Cartridge for BasicCartridge<D> {

//     fn read(&self, address: usize) -> Result<u8, MemoryError> {
//         self.data
//             .as_ref()
//             .get(address)
//             .copied()
//             .ok_or(MemoryError::Read(address))
//     }

//     fn write(&mut self, address: usize, value: u8) -> Result<(), MemoryError> {
//         match address.value() {
//             0x0000..=0x1FFF => {
//                 self.ram_enabled = value == 0x0A;
//             }
//             0x2000..=0x3FFF => {
//                 self.rom_bank = value & 0x7F;
//                 if self.rom_bank == 0 {
//                     self.rom_bank = 1;
//                 }
//             }
//             0x4000..=0x5FFF => {
//                 if self.select {
//                     self.ram_bank = value;
//                     // printf("SET RAM BANK TO 0x%02X\n", cart->ram_bank);
//                 } else {
//                     self.rom_bank = (self.rom_bank & 31) | (value << 5);
//                     // printf("SET ROM BANK (UPPER) TO 0x%02X\n", cart->rom_bank);
//                 }
//             }
//             0x6000..=0x7FFF => {
//                 if value <= 1 {
//                     self.select = value == 1;
//                 }
//             }
//             0xA000..=0xBFFF => {
//                 if self.ram_enabled {
//                     let index = self.ram_address(address);
//                     match cart.ram.get_mut(index) {
//                         Some(ptr) => Ok(*ptr = value),
//                         None => Err(CartridgeError::Ram(
//                             MemoryError::Write("Cartridge RAM", index),
//                             self.ram_bank,
//                             self.ram_enabled,
//                         )),
//                     }
//                 }
//             }
//             _ => {
//                 return Err(CartridgeError::Rom(
//                     MemoryError::Write("Cartridge ROM", address.index()),
//                     self.rom_bank,
//                 ));
//             }
//         }
//     }
// }

pub fn load(data: impl AsRef<[u8]>) -> Result<Box<dyn Cartridge + 'static>, CartridgeError> {
    let data = data.as_ref();
    if data.len() < 0x150 {
        return Err(CartridgeError::NoHeader);
    }
    let cartridge_type = data[0x147];
    Ok(match cartridge_type {
        0x00 => Box::new(mbc0::MBC0::new(data)),
        0x01..=0x03 => Box::new(mbc1::MBC1::new(data)),
        0x0F..=0x13 => Box::new(mbc3::MBC3::new(data)),
        _ => return Err(CartridgeError::NotSupported(cartridge_type)),
    })
}

pub trait Cartridge  {

    fn new(data: impl AsRef<[u8]>) -> Self where Self: Sized;

    fn title(&self) -> &str {
        str::from_utf8(&self.rom()[0x134..0x144]).unwrap_or("UNKNOWN")
    }

    fn color(&self) -> bool {
        self.read(Address::new(0x143)).expect("Invalid ROM!") & 0x80 != 0
    }

    fn read(&self, address: Address) -> Result<u8, MemoryError>;

    fn write(&mut self, address: Address, value: u8) -> Result<(), MemoryError>;

    fn rom(&self) -> &[u8];

    fn ram(&self) -> &[u8];

    fn ram_mut(&mut self) -> &mut [u8];
}
