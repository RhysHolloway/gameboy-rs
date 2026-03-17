use std::ops::{Index, IndexMut};

use crate::gb::bus::{Bus, BusError};
use crate::gb::util::Address;

#[derive(Default)]
pub struct Registers {
    bc: u16,
    de: u16,
    hl: u16,
    af: u16,
    sp: u16,
    pc: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg {
    B = 1,
    C = 0,
    D = 3,
    E = 2,
    H = 5,
    L = 4,
    /// Accumulator
    A = 7,
    /// Flags
    F = 6,
}

#[derive(Debug, Clone, Copy)]
pub enum DReg {
    BC = 0,
    DE = 1,
    HL = 2,
    /// Accumulator & Flags
    AF = 3,
    SP = 4,
    PC = 5,
}

// pub enum Flag {
//     Carry,
//     HalfCarry,
//     Negative,
//     Zero,
// }

impl Registers {

    pub const fn new() -> Self {
        Self { bc: 0, de: 0, hl: 0, af: 0,  sp: 0xFFFE, pc: 0x100, }
    }

    pub fn read_index(&self, bus: &Bus, index: u8) -> Result<u8, BusError> {
        match index {
            6 => bus.read(Address(self[DReg::HL])),
            0..=5 | 7 => Ok(self[Reg::pair(index)]),
            8.. => unreachable!(),
        }
    }

    pub fn write_index(&mut self, bus: &mut Bus, index: u8, value: u8) -> Result<(), BusError> {
        match index {
            6 => bus.write(Address(self[DReg::HL]), value),
            0..=5 | 7 => Ok(self[Reg::pair(index)] = value),
            8.. => unreachable!(),
        }
    }

    pub fn flag(&self, flag: u8) -> bool {
        (self[Reg::F] & flag) != 0
    }

    pub fn set_flag(&mut self, flag: u8, value: bool) {
        if value {
            self[Reg::F] |= flag;
        } else {
            self[Reg::F] &= !flag;
        }
    }
}

impl Reg {
    /// Bit position of the carry flag
    pub const FLAG_CARRY: u8 = 1 << 4;
    pub const FLAG_HALF_CARRY: u8 = 1 << 5;
    pub const FLAG_NEGATIVE: u8 = 1 << 6;
    pub const FLAG_ZERO: u8 = 1 << 7;

    #[inline]
    pub const fn pair(value: u8) -> Self {
        match value {
            0 => Self::B,
            1 => Self::C,
            2 => Self::D,
            3 => Self::E,
            4 => Self::H,
            5 => Self::L,
            7 => Self::A,
            6 | 8.. => unreachable!(),
        }
    }

}

impl DReg {

    #[inline]
    pub const fn pair1(p: u8) -> Self {
        match p {
            0 => DReg::BC,
            1 => DReg::DE,
            2 => DReg::HL,
            3 => DReg::SP,
            _ => unreachable!(),
        }
    } 
    
    #[inline]
    pub const fn pair2(p: u8) -> Self {
        match p {
            0 => DReg::BC,
            1 => DReg::DE,
            2 => DReg::HL,
            3 => DReg::AF,
            _ => unreachable!(),
        }
    }

}

impl Index<Reg> for Registers {
    type Output = u8;

    fn index(&self, register: Reg) -> &Self::Output {
        let value = match register {
            Reg::B | Reg::C => &self.bc,
            Reg::D | Reg::E => &self.de,
            Reg::H | Reg::L => &self.hl,
            Reg::A | Reg::F => &self.af,
        };
        &bytemuck::bytes_of(value)[register as u8 as usize & 1]
    }
}

impl IndexMut<Reg> for Registers {
    fn index_mut(&mut self, register: Reg) -> &mut Self::Output {
        let value = match register {
            Reg::B | Reg::C => &mut self.bc,
            Reg::D | Reg::E => &mut self.de,
            Reg::H | Reg::L => &mut self.hl,
            Reg::A | Reg::F => &mut self.af,
        };
        &mut bytemuck::bytes_of_mut(value)[register as u8 as usize & 1]
    }
}

impl Index<DReg> for Registers {
    type Output = u16;

    fn index(&self, register: DReg) -> &Self::Output {
        match register {
            DReg::BC => &self.bc,
            DReg::DE => &self.de,
            DReg::HL => &self.hl,
            DReg::AF => &self.af,
            DReg::SP => &self.sp,
            DReg::PC => &self.pc,
        }
    }
}

impl IndexMut<DReg> for Registers {
    fn index_mut(&mut self, register: DReg) -> &mut Self::Output {
        match register {
            DReg::BC => &mut self.bc,
            DReg::DE => &mut self.de,
            DReg::HL => &mut self.hl,
            DReg::AF => &mut self.af,
            DReg::SP => &mut self.sp,
            DReg::PC => &mut self.pc,
        }
    }
}