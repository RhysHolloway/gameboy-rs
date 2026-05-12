use web_time as time;

use crate::cartridge::{ram_banks, rom_banks};
use crate::{Address, Cartridge};

pub struct MBC3 {
    rom: Vec<u8>,
    ram: Box<[u8]>,
    ram_enabled: bool,
    rom_bank: u8,
    rom_banks: u8,
    ram_bank: u8,
    rtc: RTC,
}

struct RTC {
    rtc_ram: [u8; 5],
    rtc_ram_latch: [u8; 5],
    rtc_zero: Option<u64>,
    rtc_latch_armed: bool,
}

impl RTC {
    const HALT_BIT: u8 = 0x40;
    const DAY_COUNTER_OVERFLOW_BIT: u8 = 0x80;

    fn new(data: &[u8]) -> Self {
        let rtc = match data[0x147] {
            0x0F | 0x10 => Some(0),
            _ => None,
        };

        Self {
            rtc_ram: [0u8; 5],
            rtc_ram_latch: [0u8; 5],
            rtc_zero: rtc,
            rtc_latch_armed: false,
        }
    }

    fn latch_rtc_reg(&mut self) {
        self.calc_rtc_reg();
        self.rtc_ram_latch.clone_from_slice(&self.rtc_ram);
    }

    const fn halted(&self) -> bool {
        self.rtc_ram[4] & Self::HALT_BIT != 0
    }

    fn calc_rtc_reg(&mut self) {
        // Do not modify regs when halted
        if self.halted() {
            return;
        }

        let tzero = match self.rtc_zero {
            Some(t) => time::UNIX_EPOCH + time::Duration::from_secs(t),
            None => return,
        };

        if self.compute_difftime() == self.rtc_zero {
            // No time has passed. Do not alter registers
            return;
        }

        let difftime = match time::SystemTime::now().duration_since(tzero) {
            Ok(n) => n.as_secs(),
            _ => 0,
        };
        self.rtc_ram[0] = (difftime % 60) as u8;
        self.rtc_ram[1] = ((difftime / 60) % 60) as u8;
        self.rtc_ram[2] = ((difftime / 3600) % 24) as u8;
        let days = difftime / (3600 * 24);
        self.rtc_ram[3] = days as u8;
        self.rtc_ram[4] = (self.rtc_ram[4] & 0xFE) | (((days >> 8) & 0x01) as u8);
        if days >= 512 {
            self.rtc_ram[4] |= Self::DAY_COUNTER_OVERFLOW_BIT;
            self.rtc_zero = self.compute_difftime();
        }
    }

    fn compute_difftime(&self) -> Option<u64> {
        if self.rtc_zero.is_none() {
            return None;
        }
        let mut difftime = match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
            Ok(t) => t.as_secs(),
            Err(_) => panic!("System clock is set to a time before the unix epoch (1970-01-01)"),
        };
        difftime -= self.rtc_ram[0] as u64;
        difftime -= (self.rtc_ram[1] as u64) * 60;
        difftime -= (self.rtc_ram[2] as u64) * 3600;
        let days = ((self.rtc_ram[4] as u64 & 0x1) << 8) | (self.rtc_ram[3] as u64);
        difftime -= days * 3600 * 24;
        Some(difftime)
    }

    const fn read(&self, bank: u8) -> u8 {
        match bank {
            0x08..=0x0C => self.rtc_ram[bank as usize - 0x08],
            _ => unreachable!(),
        }
    }

    fn write(&mut self, bank: u8, value: u8) {
        self.calc_rtc_reg();
        let mask = match bank {
            8 | 9 => 0x3F,
            10 => 0x1F,
            12 => 0xC1,
            _ => 0xFF,
        };
        self.rtc_ram[bank as usize - 0x08] = value & mask;
        self.rtc_zero = self.compute_difftime();
    }

    fn latch(&mut self, value: u8) {
        if value & 1 == 0 {
            self.rtc_latch_armed = true;
        } else if self.rtc_latch_armed {
            self.latch_rtc_reg();
            self.rtc_latch_armed = false;
        }
    }
}

impl Cartridge for MBC3 {
    fn new(data: impl AsRef<[u8]>) -> Self
    where
        Self: Sized,
    {
        let data = data.as_ref();
        let subtype = data[0x147];
        let ram_banks = match subtype {
            0x10 | 0x12 | 0x13 => ram_banks(&data),
            _ => 0,
        };

        Self {
            rom: data.to_vec(),
            ram: unsafe { Box::new_zeroed_slice(ram_banks as usize * 0x2000).assume_init() },
            ram_bank: 0,
            ram_enabled: false,
            rom_bank: 1,
            rtc: RTC::new(data),
            rom_banks: rom_banks(&data) as u8,
        }
    }

    fn rom(&self) -> &[u8] {
        self.rom.as_slice()
    }

    fn ram(&self) -> &[u8] {
        &self.ram
    }

    fn ram_mut(&mut self) -> &mut [u8] {
        &mut self.ram
    }

    fn read(&self, address: Address) -> u8 {
        match address.value() {
            0x0000..=0x3FFF => self.rom[address.index()],
            0x4000..=0x7FFF => {
                self.rom[self.rom_bank as usize * 0x4000 | (address.index() & 0x3FFF)]
            }
            0xA000..=0xBFFF => self
                .ram_enabled
                .then(|| match self.ram_bank {
                    0x0..=0x07 => {
                        self.ram[self.ram_bank as usize * 0x2000 | (address.index() & 0x1FFF)]
                    }
                    0x08..=0x0C => self.rtc.read(self.ram_bank),
                    _ => unreachable!(),
                })
                .unwrap_or(u8::MAX),
            _ => unreachable!(),
        }
    }

    fn write(&mut self, address: Address, value: u8) {
        match address.value() {
            0x0000..=0x1FFF => self.ram_enabled = value & 0xF == 0x0A,
            0x2000..=0x3FFF => self.rom_bank = 1.max(value & 0x7F) % self.rom_banks,
            0x4000..=0x5FFF => match value {
                0..=0x0C => self.ram_bank = value,
                _ => (),
            },
            0x6000..=0x7FFF => {
                self.rtc.latch(value);
            }
            0xA000..=0xBFFF => {
                if self.ram_enabled {
                    match self.ram_bank {
                        0..=0x07 => {
                            self.ram
                                [self.ram_bank as usize * 0x2000 | (address.index() & 0x1FFF)] =
                                value;
                        }
                        0x08..=0x0C => {
                            self.rtc.write(self.ram_bank, value);
                        }
                        _ => unreachable!(),
                    }
                }
            }
            _ => unreachable!(),
        }
    }
}
