#[derive(Default, Debug, Clone, Copy)]
pub struct Length<const MAX: u16> {
    value: u8,
    counter: u16,
}

impl<const MAX: u16> Length<MAX> {
    pub const fn write(&mut self, value: u8) {
        self.value = value;
        self.counter = MAX - value as u16;
    }

    pub const fn trigger(&mut self) {
        if self.counter == 0 {
            self.counter = MAX;
        }
    }

    pub const fn step(&mut self, enabled: bool) -> bool {
        if enabled && self.counter > 0 {
            self.counter -= 1;
            self.counter == 0
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
        self.value = value & 0b11;
    }

    pub const fn step(&mut self, steps: u8) {
        self.step = (self.step.wrapping_add(steps)) & 0b111;
    }

    pub const fn sample(&self) -> u8 {
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

    pub const fn write(&mut self, value: u8) {
        self.value = value;
    }

    pub const fn dac_enabled(&self) -> bool {
        self.value & 0xF8 != 0
    }

    pub const fn trigger(&mut self) {
        self.volume = self.initial_volume();
        self.counter = self.period_or_8();
    }

    const fn initial_volume(&self) -> u8 {
        self.value >> 4
    }

    const fn increasing(&self) -> bool {
        (self.value & 0b1000) != 0
    }

    const fn period(&self) -> u8 {
        self.value & 0b111
    }

    const fn period_or_8(&self) -> u8 {
        match self.period() {
            0 => 8,
            period => period,
        }
    }

    pub const fn step(&mut self) {
        if self.period() == 0 {
            return;
        }

        self.counter = self.counter.saturating_sub(1);
        if self.counter == 0 {
            self.counter = self.period_or_8();
            if self.increasing() {
                if self.volume < 15 {
                    self.volume += 1;
                }
            } else if self.volume > 0 {
                self.volume -= 1;
            }
        }
    }

    pub const fn volume(&self) -> u8 {
        self.volume
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Period {
    value: u16,
    timer: usize,
    control: u8,
}

impl Period {
    pub const fn read(&self, hi: bool) -> u8 {
        if hi {
            ((self.value >> 8) as u8 & 0x07) | (self.control & 0x40)
        } else {
            self.value as u8
        }
    }

    pub const fn write(&mut self, hi: bool, value: u8) -> bool {
        if hi {
            self.value = (self.value & 0x00FF) | (((value & 0x07) as u16) << 8);
            self.control = value & 0x40;
            value & 0x80 != 0
        } else {
            self.value = (self.value & 0x0700) | value as u16;
            false
        }
    }

    pub const fn length_enabled(&self) -> bool {
        (self.control & 0x40) != 0
    }

    pub const fn trigger(&mut self, multiplier: usize) {
        self.timer = 0;
        self.reload(multiplier);
    }

    pub const fn step(&mut self, cycles: usize, multiplier: usize) -> usize {
        let mut steps = 0;
        let mut remaining = cycles;

        if self.timer == 0 {
            self.reload(multiplier);
        }

        while remaining >= self.timer {
            remaining -= self.timer;
            self.timer = 0;
            self.reload(multiplier);
            steps += 1;
        }

        self.timer -= remaining;
        steps
    }

    const fn reload(&mut self, multiplier: usize) {
        self.timer += (0x0800 - self.value as usize) * multiplier;
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Sweep {
    value: u8,
    counter: u8,
    shadow: u16,
    enabled: bool,
    used_negate: bool,
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

    const fn period_or_8(&self) -> u8 {
        match self.period() {
            0 => 8,
            period => period,
        }
    }

    pub const fn read(&self) -> u8 {
        self.value
    }

    pub const fn write(&mut self, value: u8) -> bool {
        let disabled_by_negate_clear =
            self.negate() && !Self::value_negate(value) && self.used_negate;
        self.value = value & 0x7F;
        disabled_by_negate_clear
    }

    pub const fn trigger(&mut self, period: &Period) -> bool {
        self.shadow = period.value;
        self.counter = self.period_or_8();
        self.enabled = self.period() != 0 || self.shift() != 0;
        self.used_negate = false;

        self.shift() == 0 || self.calculate().is_some()
    }

    pub const fn step(&mut self, period: &mut Period) -> bool {
        self.counter = self.counter.saturating_sub(1);
        if self.counter != 0 {
            return true;
        }

        self.counter = self.period_or_8();
        if !self.enabled || self.period() == 0 {
            return true;
        }

        let Some(next) = self.calculate() else {
            return false;
        };

        if self.shift() != 0 {
            self.shadow = next;
            period.value = next;

            if self.calculate().is_none() {
                return false;
            }
        }

        true
    }

    const fn calculate(&mut self) -> Option<u16> {
        let delta = self.shadow >> self.shift();
        let next = if self.negate() {
            self.used_negate = true;
            self.shadow.wrapping_sub(delta)
        } else {
            self.shadow.wrapping_add(delta)
        };

        if next <= 0x07FF { Some(next) } else { None }
    }

    const fn value_negate(value: u8) -> bool {
        (value & 0b1000) != 0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Noise {
    value: u8,
    timer: usize,
    lfsr: u16,
}

impl Default for Noise {
    fn default() -> Self {
        Self {
            value: 0,
            timer: 0,
            lfsr: 0x7FFF,
        }
    }
}

impl Noise {
    const DIVISORS: [usize; 8] = [8, 16, 32, 48, 64, 80, 96, 112];

    pub const fn read(&self) -> u8 {
        self.value
    }

    pub const fn write(&mut self, value: u8) {
        self.value = value;
    }

    pub const fn trigger(&mut self) {
        self.lfsr = 0x7FFF;
        self.timer = 0;
        self.reload();
    }

    pub const fn step(&mut self, cycles: usize) {
        let mut remaining = cycles;

        if self.timer == 0 {
            self.reload();
        }

        while remaining >= self.timer {
            remaining -= self.timer;
            self.timer = 0;
            self.reload();
            if self.clock_shift() >= 14 {
                continue;
            }

            let feedback = (self.lfsr ^ (self.lfsr >> 1)) & 1;
            self.lfsr = (self.lfsr >> 1) | (feedback << 14);
            if self.width_mode() {
                self.lfsr = (self.lfsr & !(1 << 6)) | (feedback << 6);
            }
        }

        self.timer -= remaining;
    }

    pub const fn sample(&self) -> u8 {
        ((self.lfsr & 1) == 0) as u8
    }

    const fn clock_shift(&self) -> u8 {
        self.value >> 4
    }

    const fn width_mode(&self) -> bool {
        (self.value & 0b1000) != 0
    }

    const fn reload(&mut self) {
        self.timer += Self::DIVISORS[(self.value & 0b111) as usize] << self.clock_shift();
    }
}
