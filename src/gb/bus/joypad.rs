#[derive(Default)]
enum Select {
    #[default]
    None,
    DPad,
    Buttons,
}


#[derive(Default)]
pub struct Joypad {
    select: Select,
    state: u8,
}

impl Joypad {

    pub const INTERRUPT_BIT: u8 = 0x10;

    pub fn read(&self) -> u8 {
        match self.select {
            Select::None => 0xF,
            Select::DPad => self.state >> 4,
            Select::Buttons => self.state & 0xF,
        }
    }

    pub fn write(&mut self, value: u8) {
        self.select = match (value & 0x30) >> 4 {
            1 => Select::DPad,
            2 => Select::Buttons, 
            _ => Select::None,
        };
    }

    pub fn update(&mut self, int: &mut u8, (control, new_state): (Controls, bool)) {
        let bit = 1u8 << control as u8;
        let previous = self.state & bit == 0;
        self.state = (self.state & !bit) | ((!new_state as u8) << control as u8);
        if previous && new_state {
            *int |= Self::INTERRUPT_BIT;
        }
    }

}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Controls {
    A = 0, B = 1, Start = 2, Select = 3, Up = 4, Down = 5, Left = 6, Right = 7
}