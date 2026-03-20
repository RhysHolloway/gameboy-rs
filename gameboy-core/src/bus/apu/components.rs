#[derive(Default, Debug, Clone, Copy)]
pub struct Length<const MASK: u8> {
    value: u8,
    counter: u8,
}

impl<const MASK: u8> Length<MASK> {
    pub const fn read(&self) -> u8 {
        self.value
    }

    pub const fn write(&mut self, value: u8) {
        self.value = value & MASK;
    }

    pub const fn length(&self) -> u8 {
        self.value
    }

    pub const fn step(&mut self, enabled: bool) -> bool {
        if enabled && self.value < MASK {
            self.value += 1;
            true
        } else {
            false
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Duty {
    value: u8,
    step: u8,
}

impl Duty {

    const DUTYS: &'static [[u8; 8]; 4] = &[
        [0, 1, 0, 0, 0, 0, 0, 0],
        [0, 1, 1, 0, 0, 0, 0, 0],
        [0, 1, 1, 1, 1, 0, 0, 0],
        [1, 0, 0, 1, 1, 1, 1, 1],
    ];
    
    pub const fn read(&self) -> u8 {
        self.value
    }

    pub const fn write(&mut self, value: u8) {
        self.value = value & 0b111;
    }

    pub const fn step(&mut self) {
        self.step = (self.step + 1) % 8;
    }

    pub const fn duty(&self) -> u8 {
        Self::DUTYS[self.value as usize][self.step as usize]
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct VolumeEnvelope {
    value: u8,
    volume: u8,
    counter: u8,
}

impl VolumeEnvelope {
    pub const fn read(&self) -> u8 {
        self.value
    }

    pub const fn write(&mut self, value: u8) -> bool {
        self.value = value;
        self.volume = Self::volume(value);
        return value & !0b111 == 0;
    }

    const fn volume(input: u8) -> u8 {
        input >> 4
    }

    const fn negate(&self) -> bool {
        (self.value & 0b1000) != 0
    }

    const fn period(&self) -> u8 {
        self.value & 0b111
    }

    pub const fn step(&mut self) {
        if self.period() == 0 {
            return;
        }

        self.counter += 1;
        if self.counter >= self.period() {
            self.counter = 0;
            if self.negate() {
                if self.volume > 0 {
                    self.volume -= 1;
                }
            } else {
                if self.volume < 15 {
                    self.volume += 1;
                }
            }
        }
    }
    
    pub const fn value(&self) -> u8 {
        self.value
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Period {
    initial: u16,
    next: Option<u16>,
    value: u16,
}

impl Period {
    pub const fn read(&self, hi: bool) -> u8 {
        if hi {
            (self.initial >> 8) as u8
        } else {
            self.initial as u8
        }
    }

    pub const fn write(&mut self, hi: bool, value: u8) {
        let update = if hi {
            (self.initial & 0x00FF) | ((value as u16) << 8)
        } else {
            (self.initial & 0xFF00) | value as u16
        };
        if self.trigger() {
            self.next = Some(update);
        } else {
            self.initial = update;
        }
    }

    pub const fn period(&self) -> u16 {
        self.value & 0x07FF
    }

    /// (enabled)
    pub const fn trigger(&self) -> bool {
        (self.initial & 0x8000) != 0
    }

    pub const fn length_enabled(&self) -> bool {
        (self.initial & 0x4000) != 0
    }

    pub const fn step(&mut self) {
        if self.trigger() {
            self.value += 1;
            if self.value > 0x7FF {
                self.value = self.initial & 0x7FF;
            }
        }
    }

    pub const fn disable(&mut self) {
        self.initial &= 0x7FFF;
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Sweep {
    value: u8,
    counter: u8,
}

impl Sweep {
    const fn period(&self) -> u8 {
        (self.value >> 4) & 0b111
    }

    const fn negate(&self) -> bool {
        (self.value & 0b1000) != 0
    }

    const fn shift(&self) -> u8 {
        self.value & 0b111
    }

    pub const fn read(&self) -> u8 {
        self.value
    }

    pub const fn write(&mut self, value: u8) {
        self.value = value;
    }

    pub const fn step(&mut self) {
        if self.period() == 0 {
            return;
        }

        self.counter += 1;
        if self.counter >= self.period() {
            self.counter = 0;
            // let change = channel.period() >> self.shift();
            // if self.negate() {
            //     channel.period.0 = channel.period.0.wrapping_sub(change);
            // } else {
            //     channel.period.0 = channel.period.0.wrapping_add(change);
            // }
        }
    }
}
