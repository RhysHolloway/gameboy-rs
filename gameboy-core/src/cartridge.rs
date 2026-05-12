mod mbc0;
mod mbc1;
mod mbc3;
mod mbc5;

use crate::Address;

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
        0x19..=0x1E => Box::new(mbc5::MBC5::new(data)),
        _ => return Err(CartridgeError::NotSupported(cartridge_type)),
    })
}

pub trait Cartridge {
    fn new(data: impl AsRef<[u8]>) -> Self
    where
        Self: Sized;

    fn title(&self) -> &str {
        str::from_utf8(&self.rom()[0x134..0x144]).unwrap_or("UNKNOWN")
    }

    fn color(&self) -> bool {
        self.read(Address::new(0x143)) & 0x80 != 0
    }

    fn read(&self, address: Address) -> u8;

    fn write(&mut self, address: Address, value: u8);

    fn rom(&self) -> &[u8];

    fn ram(&self) -> &[u8];

    fn ram_mut(&mut self) -> &mut [u8];
}

const fn ram_banks(data: &[u8]) -> u8 {
    let v = data[0x149];
    match v {
        1 | 2 => 1,
        3 => 4,
        4 => 16,
        5 => 8,
        _ => 0,
    }
}

const fn rom_banks(data: &[u8]) -> u16 {
    let v = data[0x148];
    match v {
        0..=8 => 2 << v,
        0x52 => 72,
        0x53 => 80,
        0x54 => 96,
        _ => 0,
    }
}
