use crate::util::Address;
use crate::{Cartridge, Width};

#[derive(Default)]
pub struct Cgb {
    enabled: bool,
    prepare_speed_switch: bool,
    double_speed: bool,
    ff72: u8,
    ff73: u8,
    ff74: u8,
    ff75: u8,
}

impl Cgb {
    const ADDRESS_KEY0: Width = 0xFF4C;
    const ADDRESS_KEY1: Width = 0xFF4D;

    pub fn set_enabled(&mut self, cart: &dyn Cartridge) {
        self.enabled = cart.color();
    }

    pub const fn read_mapped(&self, address: &Address) -> u8 {
        if !self.enabled {
            return u8::MAX;
        }
        match address.value() {
            Self::ADDRESS_KEY0 => u8::MAX,
            Self::ADDRESS_KEY1 => {
                0x7E | ((self.double_speed as u8) << 7) | self.prepare_speed_switch as u8
            }
            0xFF72 => self.ff72,
            0xFF73 => self.ff73,
            0xFF74 => self.ff74,
            0xFF75 => self.ff75 | 0x8F,
            _ => unreachable!(),
        }
    }

    pub const fn write_mapped(&mut self, address: &Address, value: u8) {
        if !self.enabled {
            return;
        }
        match address.value() {
            Self::ADDRESS_KEY0 => (),
            Self::ADDRESS_KEY1 => self.prepare_speed_switch = value & 1 != 0,
            0xFF72 => self.ff72 = value,
            0xFF73 => self.ff73 = value,
            0xFF74 => self.ff74 = value,
            0xFF75 => self.ff75 = value,
            _ => unreachable!(),
        }
    }

    pub fn double_speed(&self) -> bool {
        self.double_speed
    }

    pub fn disarm(&mut self) -> bool {
        if self.prepare_speed_switch {
            self.prepare_speed_switch = false;
            self.double_speed = !self.double_speed;
            true
        } else {
            false
        }
    }

    pub(crate) const fn enabled(&self) -> bool {
        self.enabled
    }
}
