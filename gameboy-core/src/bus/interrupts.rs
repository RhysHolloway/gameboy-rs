use crate::Width;
use crate::util::Address;

#[derive(Debug, Default)]
enum PauseState {
    Halt,
    Stop,
    #[default]
    None,
}

#[derive(Debug, Default)]
pub struct Interrupts {
    pub i: u8,
    pub ie: u8,
    ime: Ime,
    pause: PauseState,
    halt_bug: bool,
}

#[derive(Debug, Default)]
struct Ime {
    state: bool,
    update: Option<(bool, bool)>,
}

impl Ime {
    pub const fn set(&mut self, state: bool) {
        self.update = Some((true, state));
    }

    pub const fn cycle(&mut self) -> bool {
        let previous = self.state;
        if let Some((next, new_state)) = self.update.as_mut() {
            match *next {
                true => *next = false,
                false => {
                    self.state = *new_state;
                    self.update = None;
                }
            }
        }
        previous
    }
}

impl Interrupts {

    const fn interrupt_bits(&self) -> u8 {
        self.ie & self.i & 0x1F
    }

    pub fn interrupt(&mut self) -> InterruptState {
        let ime = self.ime.cycle();

        if self.stopped() {
            return InterruptState::Stop;
        }

        if !self.halted() && !ime {
            return InterruptState::Continue;
        }

        match self.interrupt_bits() {
            0 => {
                return match self.halted() {
                    true => InterruptState::Halt,
                    false => InterruptState::Continue,
                };
            },
            bit => {
                if !ime {
                    self.pause = PauseState::None;
                    return InterruptState::Continue;
                }

                let bit = bit.trailing_zeros() as Width;
                self.i &= !(1 << bit);
                self.pause = PauseState::None;
                self.ime.state = false;
                return InterruptState::Interrupt(Address::new(0x40 + 8 * bit));
            },
        }
    }

    pub(crate) const fn set_halt(&mut self) {
        self.pause = PauseState::Halt;
        self.halt_bug = self.interrupt_bits() != 0;
    }

    pub(crate) const fn halt_bug(&mut self) -> bool {
        let bug = self.halt_bug;
        self.halt_bug = false;
        bug
    }

    pub(crate) const fn set_stop(&mut self, value: bool) {
        self.pause = if value { PauseState::Stop } else { PauseState::None };
    }

    pub(crate) const fn set_ime(&mut self, value: bool) {
        self.ime.set(value);
    }

    #[must_use]
    pub const fn halted(&self) -> bool {
        matches!(self.pause, PauseState::Halt)
    }

    #[must_use]
    pub const fn stopped(&self) -> bool {
        matches!(self.pause, PauseState::Stop)
    }

    #[must_use]
    pub const fn ie(&self) -> u8 {
        self.ie
    }

    #[must_use]
    pub const fn ime(&self) -> bool {
        self.ime.state
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InterruptState {
    Continue,
    Interrupt(Address),
    Halt,
    Stop,
}

impl Default for InterruptState {
    fn default() -> Self {
        Self::Continue
    }
}
