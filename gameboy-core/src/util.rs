use core::ops::{Add, AddAssign, Sub, SubAssign};
use core::fmt::{UpperHex, Display, Formatter, Result};

use alloc::boxed::Box;

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
        Self(self.0.wrapping_add(rhs))
    }
}

impl AddAssign<Width> for Address {
    fn add_assign(&mut self, rhs: Width) {
        self.0 = self.0.wrapping_add(rhs);
    }
}

impl SubAssign<Width> for Address {
    fn sub_assign(&mut self, rhs: Width) {
        self.0 = self.0.wrapping_sub(rhs);
    }
}

impl Sub for Address {
    type Output = Address;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.wrapping_sub(rhs.0))
    }
}

impl Sub<Width> for Address {
    type Output = Address;

    fn sub(self, rhs: Width) -> Self::Output {
        Self(self.0.wrapping_sub(rhs))
    }
}

impl UpperHex for Address {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        UpperHex::fmt(&self.0, f)
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{:#04X}", self.0)
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
pub struct Memory<const SIZE: usize, T: Sized = u8> {
    data: Box<[T; SIZE]>,
}

impl<const SIZE: usize, T: Sized> Default for Memory<SIZE, T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize, T: Sized> Memory<SIZE, T> {
    pub const SIZE: usize = SIZE;

    pub fn new() -> Self {
        Self {
            data: unsafe { Box::new_zeroed().assume_init() },
        }
    }
}

impl<const SIZE: usize, T: Sized> Memory<SIZE, T> {
    pub const fn as_slice(&self) -> &[T; SIZE] {
        &self.data
    }

    pub const fn as_slice_mut(&mut self) -> &mut [T; SIZE] {
        &mut self.data
    }
}

impl<const SIZE: usize, T: Sized + Copy> Memory<SIZE, T> {
    pub const fn read(&self, index: usize) -> T {
        self.data[index]
    }

    pub const fn write(&mut self, index: usize, value: T) {
        self.data[index] = value;
    }
}
