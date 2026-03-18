use std::ops::{Add, AddAssign, Sub, SubAssign};

pub type Width = u16;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Address(Width);

impl Address {
    pub const fn add(self, offset: usize) -> usize {
        self.0 as usize + offset
    }

    pub const fn sub(self, offset: usize) -> usize {
        self.0 as usize - offset
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn value(self) -> u16 {
        self.0
    }

    pub const fn from_index(value: usize) -> Self {
        Self(value as Width)
    }

    pub const fn new(value: Width) -> Self {
        Self(value)
    }
}

impl Into<usize> for Address {
    fn into(self) -> usize {
        self.index()
    }
}

impl Add<Width> for Address {
    type Output = Address;

    fn add(self, rhs: Width) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<Width> for Address {

    fn add_assign(&mut self, rhs: Width) {
        self.0 += rhs;
    }
}

impl SubAssign<Width> for Address {

    fn sub_assign(&mut self, rhs: Width) {
        self.0 -= rhs;
    }
}

impl Sub for Address {
    type Output = Address;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Sub<Width> for Address {
    type Output = Address;

    fn sub(self, rhs: Width) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl std::fmt::UpperHex for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::UpperHex::fmt(&self.0, f)
    }
}

impl std::fmt::Display for Address {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#04X}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Controls {
    Right = 0,
    Left = 1,
    Up = 2,
    Down = 3,
    A = 4,
    B = 5,
    Start = 6,
    Select = 7,
}

#[derive(Clone)]
pub struct Memory<const SIZE: usize> {
    location: &'static str,
    data: Box<[u8; SIZE]>,
}

impl<const SIZE: usize> Memory<SIZE> {
    pub const SIZE: usize = SIZE;

    pub fn new(location: &'static str) -> Self {
        Self {
            location,
            data: unsafe { Box::new_zeroed().assume_init() },
        }
    }
}

impl<const SIZE: usize> Memory<SIZE> {
    pub const fn read_offset(&self, address: Address) -> Result<u8, MemoryError> {
        let index = address.index();
        if index < Self::SIZE {
            Ok(self.data[index])
        } else {
            Err(MemoryError::Read(self.location, address))
        }
    }

    pub const fn write_offset(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        let index = address.index();
        if index < Self::SIZE {
            self.data[index] = value;
            Ok(())
        } else {
            Err(MemoryError::Write(self.location, address))
        }
    }
}

#[derive(Clone)]
pub struct OffsetMemory<const START: usize, const SIZE: usize>(Memory<SIZE>);

impl<const START: usize, const SIZE: usize> OffsetMemory<START, SIZE> {
    pub const START: usize = START;
    pub const END: usize = START + (SIZE) - 1;
    pub const SIZE: usize = SIZE;

    pub fn new(location: &'static str) -> Self {
        Self(Memory::new(location))
    }

    pub const fn read_offset(&self, address: Address) -> Result<u8, MemoryError> {
        self.0.read_offset(address)
    }

    pub const fn write_offset(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        self.0.write_offset(address, value)
    }

    const fn map(&self, address: Address) -> Address {
        Address::from_index(address.sub(START))
    }

    pub const fn read_mapped(&self, address: Address) -> Result<u8, MemoryError> {
        let offset = self.map(address).index();
        if offset < Self::SIZE {
            Ok(self.0.data[offset])
        } else {
            Err(MemoryError::Read(self.0.location, address))
        }
    }

    pub const fn write_mapped(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        let offset = self.map(address).index();
        if offset < Self::SIZE {
            self.0.data[offset] = value;
            Ok(())
        } else {
            Err(MemoryError::Write(self.0.location, address))
        }
    }

    pub const fn location(&self) -> &'static str {
        self.0.location
    }

}

#[derive(Debug, Clone, Copy)]
pub struct MemoryError {
    pub location: &'static str,
    pub kind: MemoryErrorKind,
}

#[derive(Debug, Clone, Copy)]
pub enum MemoryErrorKind {
    Read(Address),
    Write(Address),
    IO(&'static str),
}

impl MemoryError {
    pub const fn Read(location: &'static str, address: Address) -> Self {
        Self {
            location,
            kind: MemoryErrorKind::Read(address),
        }
    }

    pub const fn Write(location: &'static str, address: Address) -> Self {
        Self {
            location,
            kind: MemoryErrorKind::Write(address),
        }
    }

    pub const fn IO(location: &'static str, error: &'static str) -> Self {
        Self {
            location,
            kind: MemoryErrorKind::IO(error),
        }
    }
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            MemoryErrorKind::Read(address) => write!(
                f,
                "Could not read from address {:#04X} in {}",
                address, self.location
            ),
            MemoryErrorKind::Write(address) => write!(
                f,
                "Could not write to address {:#04X} in {}",
                address, self.location
            ),
            MemoryErrorKind::IO(error) => {
                write!(f, "Could not perform IO in {}: {}", self.location, error)
            }
        }
    }
}
