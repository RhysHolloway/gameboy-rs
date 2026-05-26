mod channels;
mod components;

use core::u8;

use crate::bus::apu::channels::*;
use crate::{Address, Cycles, Width};

const FRAME_SEQUENCER_PERIOD: usize = 8192;

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
    pub const fn mix(&self) -> [u8; 2] {
        if !self.enabled {
            return [0, 0];
        }

        let channels = [
            self.ch1.output(),
            self.ch2.output(),
            self.ch3.output(),
            self.ch4.output(),
        ];

        [
            self.mix_side(&channels, 4, (self.volume_vin >> 4) & 0x07),
            self.mix_side(&channels, 0, self.volume_vin & 0x07),
        ]
    }

    const fn status(&self) -> u8 {
        (self.ch1.enabled() as u8)
            | ((self.ch2.enabled() as u8) << 1)
            | ((self.ch3.enabled() as u8) << 2)
            | ((self.ch4.enabled() as u8) << 3)
    }

    fn clear_registers(&mut self) {
        self.panning = 0;
        self.volume_vin = 0;
        self.ch1 = Channel1::default();
        self.ch2 = Channel2::default();
        self.ch3 = Channel3::default();
        self.ch4 = Channel4::default();
    }

    const fn mix_channel(&self, channel: usize, channels: &[u8; 4], panning_shift: u8) -> u16 {
        if self.panning & (1 << (channel as u8 + panning_shift)) != 0 {
            channels[channel] as u16
        } else {
            0
        }
    }

    const fn mix_side(&self, channels: &[u8; 4], panning_shift: u8, volume: u8) -> u8 {
        let mut mixed = 0u16;

        mixed += self.mix_channel(0, channels, panning_shift) as u16;
        mixed += self.mix_channel(1, channels, panning_shift) as u16;
        mixed += self.mix_channel(2, channels, panning_shift) as u16;
        mixed += self.mix_channel(3, channels, panning_shift) as u16;

        let mixed = (mixed * (volume as u16 + 1)) / 8;
        if (u8::MAX as u16) < mixed { u8::MAX } else { mixed as u8 }
    }
}

pub type AudioCallback = super::Callback<([u8; 2], usize)>;

#[derive(Default)]
pub struct APU {
    state: AudioState,
    wave_ram: [u8; 16],
    frame_counter: usize,
    frame_step: usize,
    pub(super) callback: AudioCallback,
}

impl APU {
    pub const ADDRESS_CONTROL: Width = 0xFF26;
    pub const ADDRESS_PANNING: Width = 0xFF25;
    pub const ADDRESS_VOLUME_VIN: Width = 0xFF24;

    pub(super) fn read(&self, address: &Address) -> u8 {
        let address = address.value();
        match address {
            Self::ADDRESS_CONTROL => 0x70 | ((self.state.enabled as u8) << 7) | self.state.status(),
            Self::ADDRESS_PANNING => self.state.panning,
            Self::ADDRESS_VOLUME_VIN => self.state.volume_vin,
            0xFF10..=0xFF14 => self.state.ch1.read(address - 0xFF10),
            0xFF15 | 0xFF1F => 0xFF,
            0xFF16..=0xFF19 => self.state.ch2.read(address - 0xFF16),
            0xFF1A..=0xFF1E => self.state.ch3.read(address - 0xFF1A),
            0xFF20..=0xFF23 => self.state.ch4.read(address - 0xFF20),
            0xFF30..=0xFF3F => self.wave_ram[address as usize - 0xFF30],
            _ => unreachable!(),
        }
    }

    pub(super) fn write(&mut self, address: &Address, value: u8) {
        let address = address.value();

        match address {
            0xFF30..=0xFF3F => self.wave_ram[address as usize - 0xFF30] = value,
            Self::ADDRESS_CONTROL => self.write_control(value),
            address => {
                if self.state.enabled {
                    match address {
                        Self::ADDRESS_PANNING => self.state.panning = value,
                        Self::ADDRESS_VOLUME_VIN => self.state.volume_vin = value,
                        0xFF10..=0xFF14 => self.state.ch1.write(address - 0xFF10, value),
                        0xFF15 | 0xFF1F => (),
                        0xFF16..=0xFF19 => {
                            self.state.ch2.write(address - 0xFF16, value);
                        }
                        0xFF1A..=0xFF1E => self.state.ch3.write(address - 0xFF1A, value),
                        0xFF20..=0xFF23 => self.state.ch4.write(address - 0xFF20, value),
                        _ => unreachable!(),
                    }
                }
            }
        };
    }

    pub(super) fn cycle(&mut self, cycles: &Cycles) {
        self.frame_counter += cycles.t();
        while self.frame_counter >= FRAME_SEQUENCER_PERIOD {
            self.frame_counter -= FRAME_SEQUENCER_PERIOD;
            self.frame_sequencer_step();
            self.frame_step = (self.frame_step + 1) & 0b111;
        }

        if self.state.enabled {
            self.state.ch1.cycle(cycles);
            self.state.ch2.cycle(cycles);
            self.state.ch3.cycle(cycles, &self.wave_ram);
            self.state.ch4.cycle(cycles);
        }

        if let Some(callback) = self.callback.as_mut() {
            callback((self.state.mix(), cycles.t()));
        }
    }

    pub const fn mix(&self) -> [u8; 2] {
        self.state.mix()
    }

    fn write_control(&mut self, value: u8) {
        let enabled = value & 0x80 != 0;
        match (self.state.enabled, enabled) {
            (true, false) => {
                self.state.clear_registers();
                self.state.enabled = false;
                self.frame_step = 0;
            }
            (false, true) => {
                self.state.enabled = true;
                self.frame_step = 0;
            }
            _ => (),
        }
    }

    const fn frame_sequencer_step(&mut self) {
        match self.frame_step {
            0 | 2 | 4 | 6 => {
                self.state.ch1.length();
                self.state.ch2.length();
                self.state.ch3.length();
                self.state.ch4.length();
            }
            _ => (),
        }

        match self.frame_step {
            2 | 6 => self.state.ch1.sweep(),
            _ => (),
        }

        if self.frame_step == 7 {
            self.state.ch1.envelope();
            self.state.ch2.envelope();
            self.state.ch4.envelope();
        }
    }
}
