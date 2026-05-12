use crate::bus::apu::components::*;
use crate::{Cycles, Width};

#[derive(Default, Debug, Clone, Copy)]
pub struct Channel1 {
    sweep: Sweep,
    channel: Channel2,
}

impl Channel1 {
    pub fn read(&self, offset: u16) -> u8 {
        match offset {
            0 => self.sweep.read() | 0x80,
            1..=4 => self.channel.read(offset - 1),
            _ => unreachable!(),
        }
    }

    pub fn write(&mut self, offset: u16, value: u8) {
        match offset {
            0 => {
                if self.sweep.write(value) {
                    self.channel.disable();
                }
            }
            1..=4 => {
                if self.channel.write(offset - 1, value)
                    && !self.sweep.trigger(&self.channel.period)
                {
                    self.channel.disable();
                }
            }
            _ => unreachable!(),
        }
    }

    pub const fn cycle(&mut self, cycles: &Cycles) {
        self.channel.cycle(cycles);
    }

    pub const fn length(&mut self) {
        self.channel.length();
    }

    pub const fn sweep(&mut self) {
        if !self.sweep.step(&mut self.channel.period) {
            self.channel.disable();
        }
    }

    pub const fn envelope(&mut self) {
        self.channel.envelope();
    }

    pub const fn output(&self) -> u8 {
        self.channel.output()
    }

    pub const fn enabled(&self) -> bool {
        self.channel.enabled()
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Channel2 {
    enabled: bool,
    length: Length<64>,
    duty: Duty,
    volenv: VolumeEnvelope,
    period: Period,
}

impl Channel2 {
    pub const fn read(&self, offset: u16) -> u8 {
        match offset {
            0 => (self.duty.read() << 6) | 0x3F,
            1 => self.volenv.read(),
            2 => 0xFF,
            3 => self.period.read(true) | 0xBF,
            _ => unreachable!(),
        }
    }

    pub const fn write(&mut self, offset: u16, value: u8) -> bool {
        let mut triggered = false;
        match offset {
            0 => {
                self.length.write(value & 0x3F);
                self.duty.write(value >> 6);
            }
            1 => {
                self.volenv.write(value);
                if !self.volenv.dac_enabled() {
                    self.disable();
                }
            }
            2 => {
                self.period.write(false, value);
            }
            3 => {
                triggered = self.period.write(true, value);
                if triggered {
                    self.trigger();
                }
            }
            _ => unreachable!(),
        }
        triggered
    }

    pub const fn trigger(&mut self) {
        self.length.trigger();
        self.period.trigger(4);
        self.volenv.trigger();
        self.enabled = self.volenv.dac_enabled();
    }

    pub const fn length(&mut self) {
        if self.length.step(self.period.length_enabled()) {
            self.disable();
        }
    }

    pub const fn envelope(&mut self) {
        self.volenv.step();
    }

    pub const fn cycle(&mut self, cycles: &Cycles) {
        if !self.enabled {
            return;
        }

        self.duty.step(self.period.step(cycles.t(), 4) as u8);
    }

    pub const fn output(&self) -> u8 {
        if self.enabled && self.duty.sample() != 0 && self.volenv.dac_enabled() {
            self.volenv.volume()
        } else {
            0
        }
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn disable(&mut self) {
        self.enabled = false;
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Channel3 {
    enabled: bool,
    dac_enabled: bool,
    length: Length<256>,
    volume: u8,
    period: Period,
    sample_index: u8,
    sample_buffer: u8,
}

impl Channel3 {
    pub const fn read(&self, offset: u16) -> u8 {
        match offset {
            0 => ((self.dac_enabled as u8) << 7) | 0x7F,
            1 => 0xFF,
            2 => (self.volume << 5) | 0x9F,
            3 => 0xFF,
            4 => self.period.read(true) | 0xBF,
            _ => unreachable!(),
        }
    }

    pub const fn write(&mut self, offset: u16, value: u8) {
        match offset {
            0 => {
                self.dac_enabled = (value & 0x80) != 0;
                if !self.dac_enabled {
                    self.enabled = false;
                }
            }
            1 => self.length.write(value),
            2 => self.volume = (value >> 5) & 0x03,
            3 => {
                self.period.write(false, value);
            }
            4 => {
                if self.period.write(true, value) {
                    self.trigger();
                }
            }
            _ => unreachable!(),
        }
    }

    pub const fn trigger(&mut self) {
        self.length.trigger();
        self.period.trigger(2);
        self.sample_index = 0;
        self.enabled = self.dac_enabled;
    }

    pub const fn length(&mut self) {
        if self.length.step(self.period.length_enabled()) {
            self.enabled = false;
        }
    }

    pub const fn cycle(&mut self, cycles: &Cycles, wave_ram: &[u8; 16]) {
        if !self.enabled {
            return;
        }

        let step = self.period.step(cycles.t(), 2);

        self.sample_index = (self.sample_index + step as u8) & 0x1F;
        let byte = wave_ram[(self.sample_index / 2) as usize];
        self.sample_buffer = if self.sample_index & 1 == 0 {
            byte >> 4
        } else {
            byte & 0x0F
        };
    }

    pub const fn output(&self) -> u8 {
        if !self.enabled || !self.dac_enabled {
            return 0;
        }

        match self.volume {
            0 => 0,
            1 => self.sample_buffer,
            2 => self.sample_buffer >> 1,
            3 => self.sample_buffer >> 2,
            _ => unreachable!(),
        }
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub struct Channel4 {
    enabled: bool,
    length: Length<64>,
    volenv: VolumeEnvelope,
    noise: Noise,
    control: u8,
}

impl Channel4 {
    pub const fn read(&self, offset: Width) -> u8 {
        match offset {
            0 => 0xFF,
            1 => self.volenv.read(),
            2 => self.noise.read(),
            3 => self.control | 0xBF,
            _ => unreachable!(),
        }
    }

    pub const fn write(&mut self, offset: Width, value: u8) {
        match offset {
            0 => self.length.write(value & 0x3F),
            1 => {
                self.volenv.write(value);
                if !self.volenv.dac_enabled() {
                    self.enabled = false;
                }
            }
            2 => self.noise.write(value),
            3 => {
                self.control = value & 0x40;
                if value & 0x80 != 0 {
                    self.trigger();
                }
            }
            _ => unreachable!(),
        }
    }

    pub const fn trigger(&mut self) {
        self.length.trigger();
        self.volenv.trigger();
        self.noise.trigger();
        self.enabled = self.volenv.dac_enabled();
    }

    pub const fn length(&mut self) {
        if self.length.step(self.length_enabled()) {
            self.enabled = false;
        }
    }

    pub const fn envelope(&mut self) {
        self.volenv.step();
    }

    pub const fn cycle(&mut self, cycles: &Cycles) {
        if self.enabled {
            self.noise.step(cycles.t());
        }
    }

    pub const fn output(&self) -> u8 {
        if self.enabled && self.volenv.dac_enabled() && self.noise.sample() != 0 {
            self.volenv.volume()
        } else {
            0
        }
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn length_enabled(&self) -> bool {
        (self.control & 0x40) != 0
    }
}
