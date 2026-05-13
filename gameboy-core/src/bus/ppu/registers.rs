use super::Palette;
use crate::Address;
use crate::bus::cgb::Cgb;

pub struct PpuRegisters {
    pub lcdc: u8,
    pub stat: u8,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub lyc: u8,
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
    pub wy: u8,
    pub wx: u8,
    pub opri: bool,
    pub bcp: Palette,
    pub ocp: Palette,
}

impl Default for PpuRegisters {
    fn default() -> Self {
        Self {
            lcdc: 0x91,
            stat: 0x85,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0xFF,
            obp1: 0xFF,
            wy: 0,
            wx: 0,
            opri: false,
            bcp: Default::default(),
            ocp: Default::default(),
        }
    }
}

impl PpuRegisters {
    pub const ADDRESS_LCDC: Address = Address::new(0xFF40);
    pub const ADDRESS_STAT: Address = Address::new(0xFF41); // lcd status memory location
    pub const ADDRESS_SCY: Address = Address::new(0xFF42);
    pub const ADDRESS_SCX: Address = Address::new(0xFF43);
    pub const ADDRESS_LY: Address = Address::new(0xFF44);
    pub const ADDRESS_LYC: Address = Address::new(0xFF45);
    pub const ADDRESS_BGP: Address = Address::new(0xFF47);
    pub const ADDRESS_OBJP1: Address = Address::new(0xFF48);
    pub const ADDRESS_OBJP2: Address = Address::new(0xFF49);
    pub const ADDRESS_WY: Address = Address::new(0xFF4A);
    pub const ADDRESS_WX: Address = Address::new(0xFF4B);

    pub const ADDRESS_OPRI: Address = Address::new(0xFF6C); // CGB only, Object priority register
    pub const ADDRESS_BCPS: Address = Address::new(0xFF68); // CGB only, Background color palette specification
    pub const ADDRESS_BCPD: Address = Address::new(0xFF69); // CGB only, Background color palette data
    pub const ADDRESS_OCPS: Address = Address::new(0xFF6A); // CGB only, Sprite color palette specification
    pub const ADDRESS_OCPD: Address = Address::new(0xFF6B); // CGB only, Sprite color palette data

    // interrupt flag bits
    pub const INTERRUPT_VBLANK: u8 = 1 << 0;
    /**
     * Interrupt on mode 0, 1, 2, or LY=LYC match, depending on STAT settings
     * (Also called STAT)
     */
    pub const INTERRUPT_LCD: u8 = 1 << 1;

    pub const HBLANK: u8 = 0;
    pub const VBLANK: u8 = 1;
    pub const OAM: u8 = 2;
    pub const TRANSFER: u8 = 3;

    // stat bits
    pub const STAT_LY_COMPARE: u8 = 1 << 2;
    /**
     * Interrupt on switch to mode 0 (HBlank)
     */
    pub const STAT_MODE_0_SELECT: u8 = 1 << 3;
    /**
     * Interrupt on switch to mode 1 (VBlank)
     */
    pub const STAT_MODE_1_SELECT: u8 = 1 << 4;
    /**
     * Interrupt on switch to mode 2 (OAM)
     */
    pub const STAT_MODE_2_SELECT: u8 = 1 << 5;
    /**
     * Interrupt on LY=LYC match
     */
    pub const STAT_LYC_SELECT: u8 = 1 << 6;

    //
    pub const LCDC_BG: u8 = 1;
    pub const LCDC_OBJ: u8 = 1 << 1;
    pub const LCDC_OBJ_SIZE: u8 = 1 << 2;

    pub const fn lcdc_enabled(&self) -> bool {
        self.lcdc & 0x80 != 0
    }

    pub const fn ly(&self) -> u8 {
        self.ly
    }

    pub const fn read(&self, cgb: &Cgb, address: &Address) -> u8 {
        match address {
            &Self::ADDRESS_LCDC => self.lcdc,
            &Self::ADDRESS_STAT => self.stat,
            &Self::ADDRESS_SCY => self.scy,
            &Self::ADDRESS_SCX => self.scx,
            &Self::ADDRESS_LY => self.ly(),
            &Self::ADDRESS_LYC => self.lyc,
            &Self::ADDRESS_BGP => self.bgp,
            &Self::ADDRESS_OBJP1 => self.obp0,
            &Self::ADDRESS_OBJP2 => self.obp1,
            &Self::ADDRESS_WY => self.wy,
            &Self::ADDRESS_WX => self.wx,
            other if cgb.enabled() => match other {
                &Self::ADDRESS_OPRI => self.opri as u8 | !1,
                &Self::ADDRESS_BCPS | &Self::ADDRESS_BCPD => {
                    self.bcp.read(other.value() - Self::ADDRESS_BCPS.value())
                }
                &Self::ADDRESS_OCPS | &Self::ADDRESS_OCPD => {
                    self.ocp.read(other.value() - Self::ADDRESS_OCPS.value())
                }
                _ => unreachable!(),
            },
            _ => u8::MAX,
        }
    }

    pub const fn write(&mut self, cgb: &Cgb, address: &Address, value: u8) -> bool {
        match address {
            &Self::ADDRESS_LCDC => {
                let was_enabled = self.lcdc & 0x80 != 0;
                self.lcdc = value;
                let enabled = self.lcdc & 0x80 != 0;
                if was_enabled != enabled {
                    self.stat &= 0xF8;
                    self.stat |= if was_enabled { Self::HBLANK } else { Self::OAM };
                    self.ly = 0;
                    return true;
                }
            }
            &Self::ADDRESS_STAT => self.stat = (self.stat & 0x07) | (value & 0x78),
            &Self::ADDRESS_SCY => self.scy = value,
            &Self::ADDRESS_SCX => self.scx = value,
            &Self::ADDRESS_LYC => self.lyc = value,
            &Self::ADDRESS_BGP => self.bgp = value,
            &Self::ADDRESS_OBJP1 => self.obp0 = value & 0xFC,
            &Self::ADDRESS_OBJP2 => self.obp1 = value & 0xFC,
            &Self::ADDRESS_WY => self.wy = value,
            &Self::ADDRESS_WX => self.wx = value,
            other if cgb.enabled() => match other {
                &Self::ADDRESS_OPRI => self.opri = value & 1 != 0,
                &Self::ADDRESS_BCPS | &Self::ADDRESS_BCPD => self
                    .bcp
                    .write(other.value() - Self::ADDRESS_BCPS.value(), value),
                &Self::ADDRESS_OCPS | &Self::ADDRESS_OCPD => self
                    .ocp
                    .write(other.value() - Self::ADDRESS_OCPS.value(), value),
                _ => unreachable!(),
            },
            _ => (),
        }
        false
    }

    pub const fn update_ly(&mut self, int: &mut u8, value: u8) {
        self.ly = value;
        if self.ly() == self.lyc {
            self.stat |= Self::STAT_LY_COMPARE;
            if self.stat & Self::STAT_LYC_SELECT != 0 {
                *int |= Self::INTERRUPT_LCD;
            }
        } else {
            self.stat &= !Self::STAT_LY_COMPARE;
        }
    }

    pub const fn set_mode(&mut self, int: &mut u8, mode: u8) {
        self.stat &= 0xF8;
        self.stat |= mode;
        let mut interrupt = false;
        match mode {
            Self::HBLANK => {
                interrupt = self.stat & Self::STAT_MODE_0_SELECT != 0;
            }
            Self::VBLANK => {
                *int |= Self::INTERRUPT_VBLANK;
                interrupt = self.stat & Self::STAT_MODE_1_SELECT != 0;
            }
            Self::OAM => interrupt = self.stat & Self::STAT_MODE_2_SELECT != 0,
            _ => (),
        }
        if interrupt {
            *int |= Self::INTERRUPT_LCD;
        }
    }

    pub const fn mode(&self) -> u8 {
        self.stat & 0b11
    }
}
