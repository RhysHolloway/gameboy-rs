use crate::Cycles;
use crate::util::{Address, MemoryError};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct SerialState {
    sb: u8,
    sc: u8,
    active: bool,
    cycles: u16,
    output: Option<Arc<Mutex<VecDeque<u8>>>>,
}

impl Default for SerialState {
    fn default() -> Self {
        Self {
            sb: 0x00,
            sc: 0x7E,
            active: false,
            cycles: 0,
            output: None,
        }
    }
}

impl SerialState {

    pub fn set_output(&mut self, output: Arc<Mutex<VecDeque<u8>>>) {
        self.output = Some(output);
    }

    const ADDR_SB: Address = Address::new(0xFF01);
    const ADDR_SC: Address = Address::new(0xFF02);

    const LOCATION: &'static str = "Serial IO";
    const INTERRUPT_BIT: u8 = 0b1000;

    pub(super) const fn read(&self, address: &Address) -> u8 {
        match address {
            &Self::ADDR_SB => self.sb,
            &Self::ADDR_SC => self.sc | 0x7E,
            _ => unreachable!()
        }
    }

    pub(super) const fn write(&mut self, address: &Address, value: u8)  {
        match address {
            &Self::ADDR_SB => self.sb = value,
            &Self::ADDR_SC => {
                self.sc = value & 0x83;
                if self.sc & 0x81 == 0x81 {
                    self.active = true;
                    self.cycles = 0;
                }
            }
            _ => unreachable!()
        }
    }

    pub(super) fn cycle(&mut self, int: &mut u8, cycles: &Cycles) -> Result<(), MemoryError> {
        if self.active {
            self.cycles = self.cycles.saturating_add(cycles.t() as u16);
            if self.cycles >= 4096 {
                self.active = false;
                self.sc &= 0x7F;
                if let Some(output) = self.output.as_mut() {
                    match output.lock() {
                        Ok(mut output) => output.push_back(self.sb),
                        Err(..) => return Err(MemoryError::Write(Self::LOCATION, Self::ADDR_SB)),
                    }
                }
                *int |= Self::INTERRUPT_BIT;
            }
        }
        Ok(())
    }
}