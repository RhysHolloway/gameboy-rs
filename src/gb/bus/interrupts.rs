use crate::gb::util::Address;

pub struct Interrupts {
    pub i: u8,
    pub ie: u8,
    ime: ImeState,
    halt: bool,
}

impl Default for Interrupts {
    fn default() -> Self {
        Self {
            i: 0,
            ie: 0,
            ime: Default::default(),
            halt: false,
        }
    }
}

struct ImeState {
    current: bool,
    next: (u8, bool),
}

impl Default for ImeState {
    fn default() -> Self {
        Self {
            current: true,
            next: Default::default(),
        }
    }
}

impl Interrupts {
    pub fn cycle(&mut self) {
        if self.ime.next.0 > 0 {
            if self.ime.next.0 == 1 {
                self.ime.current = self.ime.next.1;
            }
            self.ime.next.0 -= 1;
        }
    }

    pub fn interrupt(&mut self) -> InterruptState {
        if self.halt {
            return InterruptState::Halt
        } else if self.ime.current {
            let bits = self.ie & self.i;
            for bit in 0..5 {
                if bits & (1 << bit) != 0 {
                    self.halt = false;
                    self.i &= !(1 << bit);
                    return InterruptState::Interrupt(Address(0x40 + 8 * bit));
                }
            }
        }
        InterruptState::None
    }
    
    pub fn halt(&mut self) {
        self.halt = true;
    }
    
    pub fn set_ime(&mut self, value: bool) {
        self.ime.next = (2, value);
    }

    pub fn is_halting(&self) -> bool {
        self.halt
    }

    pub fn ie(&self) -> u8 {
        self.ie
    }

    pub fn ime(&self) -> bool {
        self.ime.current
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InterruptState {
    Interrupt(Address),
    Halt,
    None,
}
