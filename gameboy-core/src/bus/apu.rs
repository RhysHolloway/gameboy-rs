mod components;

use crate::bus::apu::components::*;
use crate::{Address, Cycles, MemoryError, Width};

const LOCATION: &str = "Audio";

#[derive(Default, Debug, Clone, Copy)]
pub struct AudioState {
    enabled: bool,
    panning: u8,
    volume_vin: u8,
    ch1: Channel1,
    ch2: Channel2,
    ch3: Channel3,
    ch4: Channel4,
}

impl AudioState {
    pub fn mix(&self) -> [u8; 2] {
        let ch1 = self.ch1.output();
        let ch2 = self.ch2.output();
        let ch3 = self.ch3.output();
        let ch4 = self.ch4.output();

        let channels = [ch1, ch2, ch3, ch4];

        let mut sides = [0u8; 2];

        for side in 1..=2 {
            for (channel, value) in channels.into_iter().enumerate() {
                if self.panning & (1 << (side * channel - 1)) != 0 && value != 0 {
                    sides[side - 1] += value;
                }
            }
        }

        sides.iter_mut().enumerate().for_each(|(i, val)| *val >>= 8 - ((self.volume_vin >> (i * 4)) & 0x7));

        sides
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct Channel1 {
    sweep: Sweep,
    channel: Channel2,
}

impl Channel1 {
    pub const fn read(&self, address: u16) -> u8 {
        match address {
            0 => self.sweep.read(),
            1..=4 => self.channel.read(address - 1),
            _ => unreachable!(),
        }
    }

    pub const fn write(&mut self, offset: u16, value: u8) {
        match offset {
            0 => self.sweep.write(value),
            1..=4 => self.channel.write(offset - 1, value),
            _ => unreachable!(),
        }
    }

    const fn cycle(&mut self, cycles: &Cycles) {
        self.channel.cycle(cycles);
    }

    const fn long_cycle(&mut self, step: usize) {
        if step & 0b010 != 0 {
            self.sweep.step();
        }
        self.channel.long_cycle(step);
    }

    const fn output(&self) -> u8 {
        self.channel.output() // >> self.sweep.shift()
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct Channel2 {
    enabled: bool,
    length: Length<0x3F>,
    duty: Duty,
    volenv: VolumeEnvelope,
    period: Period,
}

impl Channel2 {
    pub const fn read(&self, address: u16) -> u8 {
        match address {
            0 => self.length.read() | (self.duty.read() << 6),
            1 => self.volenv.read(),
            2 | 3 => self.period.read(address == 3),
            _ => unreachable!(),
        }
    }

    pub const fn write(&mut self, offset: u16, value: u8) {
        match offset {
            0 => {
                self.length.write(value & 0x3F);
                self.duty.write(value >> 6);
            }
            1 => {
                if self.volenv.write(value) {
                    self.enabled = false;
                }
            }
            2 | 3 => self.period.write(offset == 3, value),
            _ => unreachable!(),
        }
    }

    pub const fn long_cycle(&mut self, cycle: usize) {
        if cycle & 1 == 0 {
            self.period.step();
        }
        if cycle == 7 {
            self.volenv.step();
        }
    }

    pub const fn cycle(&mut self, cycles: &Cycles) {}

    pub const fn output(&self) -> u8 {
        if self.period.trigger() && self.duty.duty() != 0 {
            self.volenv.value()
        } else {
            0
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct Channel3 {
    enabled: bool,
    length: Length<0xFF>,
    volume: u8,
    wave_ram: [u8; 16],
    period: Period,
}

impl Channel3 {
    pub const fn read(&self, offset: u16) -> u8 {
        match offset {
            0 => self.enabled as u8,
            1 => self.length.read(),
            2 => self.volume << 5,
            3 | 4 => self.period.read(offset == 4),
            0x16..=0x25 => self.wave_ram[offset as usize - 0x10],
            _ => unreachable!(),
        }
    }

    pub const fn write(&mut self, offset: u16, value: u8) {
        match offset {
            0 => self.enabled = (value & 0x80) != 0,
            1 => self.length.write(value),
            2 => self.volume = (value >> 5) & 0x3,
            3 | 4 => self.period.write(offset == 4, value),
            0x16..=0x25 => self.wave_ram[offset as usize - 0x10] = value,
            _ => unreachable!(),
        }
    }

    pub const fn long_cycle(&mut self, step: usize) {
        if step & 1 == 0 {
            self.length.step(self.period.length_enabled());
        }
    }

    pub const fn cycle(&mut self, cycles: &Cycles) {}

    pub const fn output(&self) -> u8 {
        if self.enabled && self.volume != 0 {
            0 >> self.volume.saturating_sub(1)
        } else {
            0
        }
    }
}

#[derive(Default, Debug, Clone, Copy)]
struct Channel4 {
    enabled: bool,
    length: Length<0x3F>,
    volenv: VolumeEnvelope,
    period: Period,
}

impl Channel4 {
    pub const START: Width = 0xFF1F;

    pub const fn read(&self, address: Width) -> Result<u8, MemoryError> {
        Ok(match address {
            // 0 => self.length, // length unreadable
            1 => self.volenv.read(),
            2 | 3 => self.period.read(address == 3),
            _ => {
                return Err(MemoryError::Read(
                    LOCATION,
                    (Self::START + address) as usize,
                ));
            }
        })
    }

    pub const fn write(&mut self, offset: Width, value: u8) {
        match offset {
            0 => self.length.write(value & 0x3F),
            1 => {
                self.volenv.write(value);
            }
            2 | 3 => self.period.write(offset == 3, value),
            _ => unreachable!(),
        }
    }

    pub const fn long_cycle(&mut self, step: usize) {
        if step & 1 == 0 {
            self.length.step(self.period.length_enabled());
        }
        self.period.step();
        if step == 7 {
            self.volenv.step();
        }
    }

    pub const fn cycle(&mut self, cycles: &Cycles) {
        if !self.period.trigger() {
            return;
        }
    }

    const fn envelope(&mut self) {
        self.volenv.step();
    }
    
    const fn output(&self) -> u8 {
        if self.enabled {
            self.volenv.value()
        } else {
            0
        }
    }
}

#[derive(Default)]
pub struct APU {
    state: AudioState,
    counter: usize,
    callback: Option<Box<dyn FnMut([f32; 2]) + 'static>>,
}

impl APU {
    pub const ADDRESS_CONTROL: Width = 0xFF26;
    pub const ADDRESS_PANNING: Width = 0xFF25;
    pub const ADDRESS_VOLUME_VIN: Width = 0xFF24;

    pub(super) const fn read(&self, address: &Address) -> Result<u8, MemoryError> {
        let address = address.value();
        Ok(match address {
            Self::ADDRESS_CONTROL => {
                (self.state.enabled as u8 >> 7)
                    | (self.state.ch4.enabled as u8 >> 3)
                    | (self.state.ch3.enabled as u8 >> 2)
                    | (self.state.ch2.enabled as u8 >> 1)
                    | (self.state.ch1.channel.enabled as u8)
            }
            Self::ADDRESS_PANNING => self.state.panning,
            Self::ADDRESS_VOLUME_VIN => self.state.volume_vin,
            0xFF10..=0xFF14 => self.state.ch1.read(address - 0xFF10),
            0xFF16..=0xFF19 => self.state.ch2.read(address - 0xFF16),
            0xFF1A..=0xFF1E | 0xFF30..=0xFF3F => self.state.ch3.read(address - 0xFF1A),
            0xFF1F..=0xFF23 => match self.state.ch4.read(address - 0xFF1F) {
                Ok(value) => value,
                Err(e) => return Err(e),
            },
            _ => unreachable!(),
        })
    }

    pub(super) const fn write(&mut self, address: &Address, value: u8) {
        let address = address.value();
        match address {
            Self::ADDRESS_CONTROL => self.state.enabled = value & 0x80 != 0,
            Self::ADDRESS_PANNING => self.state.panning = value,
            Self::ADDRESS_VOLUME_VIN => self.state.volume_vin = value,
            0xFF10..=0xFF14 => self.state.ch1.write(address - 0xFF10, value),
            0xFF16..=0xFF19 => self.state.ch2.write(address - 0xFF16, value),
            0xFF1A..=0xFF1E => self.state.ch3.write(address - 0xFF1A, value),
            0xFF20..=0xFF23 => self.state.ch4.write(address - 0xFF20, value),
            _ => unreachable!(),
        }
    }

    pub(super) const fn cycle(&mut self, cycles: &Cycles) {
        self.counter += cycles.t() * 8;

        while self.counter / 8 >= 8192 {
            self.counter -= 8 * 8192;
            let step = self.counter & 7;
            self.state.ch1.long_cycle(step);
            self.state.ch2.long_cycle(step);
            self.state.ch3.long_cycle(step);
            self.state.ch4.long_cycle(step);
        }

        self.state.ch1.cycle(cycles);
        self.state.ch2.cycle(cycles);
        self.state.ch3.cycle(cycles);
        self.state.ch4.cycle(cycles);
    }
    
    pub fn mix(&self) -> [u8; 2] {
        self.state.mix()
    }
}
