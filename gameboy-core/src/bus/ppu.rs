use std::cmp::Ordering;
use std::ops::Deref;

use crate::bus::cgb::Cgb;
use crate::util::{Address, MemoryError, OffsetMemory};
use crate::{Cycles, Width};

mod cdma;
mod dma;

pub use cdma::Cdma;
pub use dma::Dma;

pub struct Ppu {
    clock: u16,
    vram: OffsetMemory<0x8000, { Self::VRAM_BANK_SIZE as usize * 2 }>,
    vram_bank: bool,
    voam: OffsetMemory<0xFE00, 0xA0>,
    framebuffer: Box<[Pixel; Self::SCREEN_WIDTH * Self::SCREEN_HEIGHT]>,
    lcdc: u8,
    stat: u8,
    scy: u8,
    scx: u8,
    pub ly: u8,
    lyc: u8,
    bgp: u8,
    obp0: u8,
    obp1: u8,
    wy: u8,
    wx: u8,
    opri: bool,
    bcps: u8,
    bcpd: [[[u8; 3]; 4]; 8],
    ocps: u8,
    ocpd: [[[u8; 3]; 4]; 8],
    //
    window_line: u8,
}

#[repr(C)]
#[derive(Default)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Pixel {
    pub const fn monochrome(palette: u8, color: u8) -> Self {
        const fn palette_color(palette: u8, color: u8) -> u8 {
            let shift = (color & 0x03) * 2;
            (palette >> shift) & 0x03
        }

        match palette_color(palette, color) {
            0 => Self {
                r: 0xE0,
                g: 0xF8,
                b: 0xD0,
            },
            1 => Self {
                r: 0x88,
                g: 0xC0,
                b: 0x70,
            },
            2 => Self {
                r: 0x34,
                g: 0x68,
                b: 0x56,
            },
            3 => Self {
                r: 0x08,
                g: 0x18,
                b: 0x20,
            },
            _ => panic!("Invalid monochrome color"),
        }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: ((r.wrapping_mul(13) + g.wrapping_mul(2) + b) >> 1) as u8,
            g: ((g.wrapping_mul(3) + b) << 1) as u8,
            b: ((r.wrapping_mul(3) + g.wrapping_mul(2) + b.wrapping_mul(11)) >> 1) as u8,
        }
    }
}

impl Deref for Pixel {
    type Target = [u8; 3];

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self as *const Pixel as *const [u8; 3]) }
    }
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            clock: 0,
            vram: OffsetMemory::new("Video RAM"),
            vram_bank: false,
            voam: OffsetMemory::new("Video OAM"),
            framebuffer: unsafe { Box::new_zeroed().assume_init() },
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
            window_line: 0,
            opri: false,
            bcps: 0,
            bcpd: Default::default(),
            ocps: 0,
            ocpd: Default::default(),
        }
    }
}

impl Ppu {
    pub const SCREEN_WIDTH: usize = 160;
    pub const SCREEN_HEIGHT: usize = 144;

    pub const VRAM_BANK_SIZE: Width = 0x2000;

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

    pub const ADDRESS_VBK: Address = Address::new(0xFF4F); // CGB only, VRAM bank select

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

    pub fn read_vram(&self, address: Address) -> Result<u8, MemoryError> {
        let address = address + (self.vram_bank as Width * Self::VRAM_BANK_SIZE);
        self.vram.read_mapped(address)
    }

    pub fn write_vram(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        self.vram.write_mapped(
            address + (self.vram_bank as Width * Self::VRAM_BANK_SIZE),
            value,
        )
    }

    pub fn read_voam<const DMA: bool>(&self, address: Address) -> Result<u8, MemoryError> {
        if !DMA && (self.mode() == Self::VBLANK || self.mode() == Self::HBLANK) {
            // return Err(MemoryError::Write("OAM during transfer", address.index()));
        }
        self.voam.read_mapped(address)
    }

    pub fn write_voam<const DMA: bool>(&mut self, address: Address, value: u8) -> Result<(), MemoryError> {
        if !DMA && (self.mode() == Self::VBLANK || self.mode() == Self::HBLANK) {
            // return Err(MemoryError::Write("OAM during transfer", address.index()));
        }
        self.voam.write_mapped(address, value)
    }

    pub fn cycle(&mut self, int: &mut u8, cgb: &Cgb, cycles: &Cycles) -> Result<bool, MemoryError> {
        if self.lcdc & 0x80 == 0 {
            return Ok(false);
        }

        let mut render = false;
        for _ in 0..cycles.t() {
            self.clock += 1;
            match self.mode() {
                Self::HBLANK => {
                    if self.clock >= 204 {
                        self.clock = 0;
                        self.update_ly(int, self.ly().wrapping_add(1));
                        if self.ly() == Self::SCREEN_HEIGHT as u8 {
                            self.set_mode(int, Self::VBLANK)?;
                            render = true;
                        } else {
                            self.set_mode(int, Self::OAM)?;
                        }
                    }
                }
                Self::VBLANK => {
                    if self.clock >= 456 {
                        self.clock = 0;
                        self.update_ly(int, self.ly().wrapping_add(1));
                        if self.ly() >= Self::SCREEN_HEIGHT as u8 + 10 {
                            self.update_ly(int, 0);
                            self.window_line = 0;
                            self.set_mode(int, Self::OAM)?;
                        }
                    }
                }
                Self::OAM => {
                    if self.clock >= 80 {
                        self.clock = 0;
                        self.set_mode(int, Self::TRANSFER)?;
                    }
                }
                Self::TRANSFER => {
                    if self.clock >= 172 {
                        if self.ly() < Self::SCREEN_HEIGHT as u8 {
                            self.draw_dmg(cgb)?;
                        }
                        self.clock = 0;
                        self.set_mode(int, Self::HBLANK)?;
                    }
                }
                _ => unreachable!(),
            }
        }
        Ok(render)
    }

    pub const fn read_reg(&self, cgb: &Cgb, address: &Address) -> Result<u8, MemoryError> {
        Ok(match address {
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
            other => match cgb.enabled() {
                true => match other {
                    &Self::ADDRESS_VBK => self.vram_bank as u8 | !1,
                    &Self::ADDRESS_OPRI => self.opri as u8 | !1,
                    &Self::ADDRESS_BCPS => 0x40 | self.bcps,
                    &Self::ADDRESS_BCPD => Self::read_palette(self.bcps, &self.bcpd),
                    &Self::ADDRESS_OCPS => 0x40 | self.ocps,
                    &Self::ADDRESS_OCPD => Self::read_palette(self.ocps, &self.ocpd),
                    _ => return Err(MemoryError::Read("PPU Register", address.index())),
                },
                false => return Err(MemoryError::Read("PPU Register", address.index())),
            },
        })
    }

    const fn read_palette(select: u8, palette: &[[[u8; 3]; 4]; 8]) -> u8 {
        let (select, palette) = Self::get_palette(select, palette);
        if select {
            palette[0] | ((palette[1] & 0x07) << 5)
        } else {
            ((palette[1] & 0x18) >> 3) | (palette[2] << 2)
        }
    }

    const fn write_palette(select: &mut u8, palette: &mut [[[u8; 3]; 4]; 8], value: u8) {
        let (lower, palette) = Self::get_palette_mut(*select, palette);
        if lower {
            palette[0] = value & 0x1F;
            palette[1] = (palette[1] & 0x18) | (value >> 5);
        } else {
            palette[1] = (palette[1] & 0x07) | ((value & 0x3) << 3);
            palette[2] = (value >> 2) & 0x1F;
        }
        if *select & 0x80 != 0 {
            *select = (*select + 1) & 0x3F;
        }
    }

    const fn get_palette(select: u8, array: &[[[u8; 3]; 4]; 8]) -> (bool, &[u8; 3]) {
        let select = select & 0x7F;
        let palette = (select >> 3) as usize;
        let col = ((select >> 1) & 0x3) as usize;
        (select & 0x01 == 0, &array[palette][col])
    }

    const fn get_palette_mut(select: u8, array: &mut [[[u8; 3]; 4]; 8]) -> (bool, &mut [u8; 3]) {
        let select = select & 0x7F;
        let palette = (select >> 3) as usize;
        let col = ((select >> 1) & 0x3) as usize;
        (select & 0x01 == 0, &mut array[palette][col])
    }

    pub const fn ly(&self) -> u8 {
        self.ly
    }

    pub const fn write_reg(
        &mut self,
        cgb: &Cgb,
        address: &Address,
        value: u8,
    ) -> Result<(), MemoryError> {
        match address {
            &Self::ADDRESS_LCDC => {
                let was_enabled = self.lcdc & 0x80 != 0;
                self.lcdc = value;
                let enabled = self.lcdc & 0x80 != 0;
                if was_enabled != enabled {
                    self.stat &= 0xF8;
                    self.stat |= if was_enabled { Self::HBLANK } else { Self::OAM };
                    self.clock = 0;
                    self.ly = 0;
                    self.window_line = 0;
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
            other => match cgb.enabled() {
                true => match other {
                    &Self::ADDRESS_VBK => self.vram_bank = value & 1 != 0,
                    &Self::ADDRESS_OPRI => self.opri = value & 1 != 0,
                    &Self::ADDRESS_BCPS => self.bcps = value & (0x3F | 0x80),
                    &Self::ADDRESS_OCPS => self.ocps = value & (0x3F | 0x80),
                    &Self::ADDRESS_BCPD => {
                        Self::write_palette(&mut self.bcps, &mut self.bcpd, value)
                    }
                    &Self::ADDRESS_OCPD => {
                        Self::write_palette(&mut self.ocps, &mut self.ocpd, value)
                    }
                    _ => return Err(MemoryError::Write("PPU Register", address.index())),
                },
                false => return Err(MemoryError::Write("PPU Register", address.index())),
            },
        }
        Ok(())
    }

    const fn update_ly(&mut self, int: &mut u8, value: u8) {
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

    const fn set_mode(&mut self, int: &mut u8, mode: u8) -> Result<(), MemoryError> {
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
        Ok(())
    }
    fn draw_dmg(&mut self, cgb: &Cgb) -> Result<(), MemoryError> {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        enum PrioType {
            Color0,
            PrioFlag,
            Normal,
        }
        #[derive(Clone, Copy, Debug)]
        struct Sprite {
            x: i16,
            y: i16,
            tile: u8,
            attrs: u8,
            oam_index: usize,
        }

        let y = self.ly() as u16;
        let bg_enable = self.lcdc & 0x01 != 0;
        let sprite_enable = self.lcdc & 0x02 != 0;
        let sprite_size = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
        let bg_map_base = if self.lcdc & 0x08 != 0 {
            0x1C00
        } else {
            0x1800
        };
        let tile_data_base = if self.lcdc & 0x10 != 0 {
            0x0000
        } else {
            0x1000
        };
        let window_enable = self.lcdc & 0x20 != 0;
        let window_map_base = if self.lcdc & 0x40 != 0 {
            0x1C00
        } else {
            0x1800
        };

        let mut sprites: [Sprite; 10] = [Sprite {
            x: 0,
            y: 0,
            tile: 0,
            attrs: 0,
            oam_index: 0,
        }; 10];
        let mut sprite_count = 0;
        if sprite_enable {
            for i in 0..40 {
                let base = i * 4;
                let sy = self.voam.read_offset(Address::from_index(base))? as i16 - 16;
                let sx = self.voam.read_offset(Address::from_index(base + 1))? as i16 - 8;
                let tile = self.voam.read_offset(Address::from_index(base + 2))?;
                let attrs = self.voam.read_offset(Address::from_index(base + 3))?;
                if y as i16 >= sy && (y as i16) < sy + sprite_size && sprite_count < 10 {
                    sprites[sprite_count] = Sprite {
                        x: sx,
                        y: sy,
                        tile,
                        attrs,
                        oam_index: i,
                    };
                    sprite_count += 1;
                }
            }
            sprites[..sprite_count].sort_unstable_by(|a, b| {
                if !cgb.enabled() && a.x != b.x {
                    return b.x.cmp(&a.x);
                }
                return b.oam_index.cmp(&a.oam_index);
            });
        }

        let mut window_drawn = false;
        for x in 0..Self::SCREEN_WIDTH as u16 {
            let mut final_color = None;
            let mut priority = PrioType::Normal;
            if bg_enable {
                let mut use_window = false;
                let wx = self.wx.wrapping_sub(7);
                if window_enable && y >= self.wy as u16 && x >= wx as u16 && self.wx <= 166 {
                    use_window = true;
                    window_drawn = true;
                }

                let (map_base, pixel_x, pixel_y) = if use_window {
                    let wx = self.wx.wrapping_sub(7) as u16;
                    let px = x - wx;
                    let py = (self.window_line as u16).wrapping_sub(0);
                    (window_map_base, px, py)
                } else {
                    let px = x.wrapping_add(self.scx as u16);
                    let py = y.wrapping_add(self.scy as u16);
                    (bg_map_base, px, py)
                };

                let tile_x = (pixel_x / 8) & 31;
                let tile_y = (pixel_y / 8) & 31;

                let tile_index_addr = Address::new(map_base + tile_y * 32 + tile_x);
                let tile_index = self.vram.read_offset(tile_index_addr)?;

                let mut tile_addr = if self.lcdc & 0x10 != 0 {
                    tile_data_base + (tile_index as u16) * 16
                } else {
                    let signed = tile_index as i8 as i16;
                    (0x1000i16 + signed * 16) as u16
                };

                let (palnr, xflip, yflip, prio) = if cgb.enabled() {
                    let flags = self
                        .vram
                        .read_offset(tile_index_addr + Self::VRAM_BANK_SIZE)?;

                    if flags & (1 << 3) != 0 {
                        tile_addr += Self::VRAM_BANK_SIZE;
                    }
                    (
                        flags & 0x07,
                        flags & (1 << 5) != 0,
                        flags & (1 << 6) != 0,
                        flags & (1 << 7) != 0,
                    )
                } else {
                    (0, false, false, false)
                };

                let line = pixel_y % 8;
                let bit = 7 - (pixel_x % 8);

                let color_addr: Address = Address::new(match yflip {
                    false => tile_addr + (line * 2),
                    true => tile_addr + (14 - (line * 2)),
                });

                let lo = self.vram.read_offset(color_addr)?;
                let hi = self.vram.read_offset(color_addr + 1)?;

                let col = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);

                if cgb.enabled() {
                    priority = if col == 0 {
                        PrioType::Color0
                    } else if prio {
                        PrioType::PrioFlag
                    } else {
                        PrioType::Normal
                    };
                }

                final_color = Some(Pixel::monochrome(self.bgp, col));
            }

            if priority == PrioType::Normal && sprite_enable && sprite_count > 0 {
                let mut sprite_pixel: Option<(u8, u8, bool)> = None;
                for sprite in sprites.iter().take(sprite_count) {
                    if x as i16 >= sprite.x && (x as i16) < sprite.x + 8 {
                        let mut sprite_y = y as i16 - sprite.y;
                        if sprite.attrs & 0x40 != 0 {
                            sprite_y = (sprite_size - 1) - sprite_y;
                        }
                        let mut sprite_x = x as i16 - sprite.x;
                        if sprite.attrs & 0x20 != 0 {
                            sprite_x = 7 - sprite_x;
                        }
                        let mut tile = sprite.tile;
                        if sprite_size == 16 {
                            tile &= 0xFE;
                            if sprite_y >= 8 {
                                tile = tile.wrapping_add(1);
                                sprite_y -= 8;
                            }
                        }
                        let tile_addr = (tile as u16) * 16;
                        let line = sprite_y as u16;
                        let bit = 7 - (sprite_x as u16);
                        let c_vram1 = if sprite.attrs & (1 << 3) != 0 && cgb.enabled() {
                            Self::VRAM_BANK_SIZE
                        } else {
                            0
                        };
                        let lo = self
                            .vram
                            .read_offset(Address::new(c_vram1 + tile_addr + line * 2))?;
                        let hi = self
                            .vram
                            .read_offset(Address::new(c_vram1 + tile_addr + line * 2 + 1))?;
                        let color_id = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);
                        if color_id != 0 {
                            let palette = if sprite.attrs & 0x10 != 0 {
                                self.obp1
                            } else {
                                self.obp0
                            };
                            let priority = sprite.attrs & 0x80 != 0;
                            sprite_pixel = Some((color_id, palette, priority));
                            break;
                        }
                    }
                }
                if let Some((color_id, palette, priority)) = sprite_pixel {
                    if !priority || final_color.is_none() {
                        final_color = Some(Pixel::monochrome(palette, color_id));
                    }
                }
            }
            self.framebuffer[self.ly() as usize * Self::SCREEN_WIDTH + x as usize] =
                final_color.unwrap_or_default();
        }

        if window_drawn {
            self.window_line = self.window_line.wrapping_add(1);
        }

        Ok(())
    }

    fn draw(&mut self, cgb: &Cgb) -> Result<(), MemoryError> {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        enum PrioType {
            Color0,
            PrioFlag,
            Normal,
        }

        let mut bgprio = [PrioType::Normal; Self::SCREEN_WIDTH];

        let bg_enable = cgb.enabled() || self.lcdc & 0x01 != 0;

        // let wx_trigger = self.wx <= 166;
        // let winy = if  self.lcdc & 0x20 != 0 && self.wy_trigger && wx_trigger {
        //     self.wy_pos += 1;
        //     self.wy_pos
        // } else {
        //     -1
        // };

        let bg_map_base = if self.lcdc & 0x08 != 0 {
            0x1C00
        } else {
            0x1800
        };

        let tile_data_base = if self.lcdc & 0x10 != 0 {
            0x0000
        } else {
            0x1000
        };

        let window_map_base = if self.lcdc & 0x40 != 0 {
            0x1C00
        } else {
            0x1800
        };

        let wintiley = (self.wy as u16 >> 3) & 31;

        let bgy = self.scy.wrapping_add(self.ly());
        let bgtiley = (bgy as u16 >> 3) & 31;

        for x in 0..Self::SCREEN_WIDTH {
            let winx = -((self.wx as i32) - 7) + (x as i32);
            let bgx = self.scx as u32 + x as u32;

            let (tilemapbase, tiley, tilex, pixely, pixelx) = if self.wy >= 0 && winx >= 0 {
                (
                    window_map_base,
                    wintiley,
                    (winx as u16 >> 3),
                    self.wy as u16 & 0x07,
                    winx as u8 & 0x07,
                )
            } else if bg_enable {
                (
                    bg_map_base,
                    bgtiley,
                    (bgx as u16 >> 3) & 31,
                    bgy as u16 & 0x07,
                    bgx as u8 & 0x07,
                )
            } else {
                continue;
            };

            let tilenraddr = Address::new(tilemapbase + tiley * 32 + tilex);
            let tilenr: u8 = self.vram.read_offset(tilenraddr)?;

            let (palnr, vram1, xflip, yflip, prio) = if cgb.enabled() {
                let flags = self.vram.read_offset(tilenraddr + Self::VRAM_BANK_SIZE)?;
                (
                    flags & 0x07,
                    flags & (1 << 3) != 0,
                    flags & (1 << 5) != 0,
                    flags & (1 << 6) != 0,
                    flags & (1 << 7) != 0,
                )
            } else {
                (0, false, false, false, false)
            };

            let tileaddress = tile_data_base
                + (if tile_data_base == 0x0000 {
                    tilenr as u16
                } else {
                    (tilenr as i8 as i16 + 128) as u16
                }) * 16;

            let a0 = Address::new(match yflip {
                false => tileaddress + (pixely * 2),
                true => tileaddress + (14 - (pixely * 2)),
            });

            let (b1, b2) = match vram1 {
                false => (self.vram.read_offset(a0)?, self.vram.read_offset(a0 + 1)?),
                true => (
                    self.vram.read_offset(a0 + Self::VRAM_BANK_SIZE)?,
                    self.vram.read_offset(a0 + 1 + Self::VRAM_BANK_SIZE)?,
                ),
            };

            let xbit = match xflip {
                true => pixelx,
                false => 7 - pixelx,
            } as u32;
            let colnr = if b1 & (1 << xbit) != 0 { 1 } else { 0 }
                | if b2 & (1 << xbit) != 0 { 2 } else { 0 };

            bgprio[x] = if colnr == 0 {
                PrioType::Color0
            } else if prio {
                PrioType::PrioFlag
            } else {
                PrioType::Normal
            };

            if cgb.enabled() {
                let palnr = palnr as usize;
                let r = self.bcpd[palnr][colnr][0];
                let g = self.bcpd[palnr][colnr][1];
                let b = self.bcpd[palnr][colnr][2];
                self.framebuffer[x as usize] = Pixel::rgb(r, g, b);
            } else {
                self.framebuffer[x as usize] = Pixel::monochrome(palnr, colnr as u8);
            }
        }

        #[derive(Clone, Copy, Debug)]
        struct Sprite {
            x: i16,
            y: i16,
            tile: u8,
            attrs: u8,
            oam_index: u8,
        }

        let sprite_enable = self.lcdc & 0x02 != 0;

        if sprite_enable {
            let sprite_size = if self.lcdc & 0x04 != 0 { 16 } else { 8 };
            let mut sprites: [Sprite; 10] = [Sprite {
                x: 0,
                y: 0,
                tile: 0,
                attrs: 0,
                oam_index: 0,
            }; 10];
            let mut sprite_count = 0;
            for i in 0..40u8 {
                let base = i as u16 * 4;
                let sy = self.voam.read_offset(Address::new(base))? as i16 - 16;
                let sx = self.voam.read_offset(Address::new(base + 1))? as i16 - 8;
                let tile = self.voam.read_offset(Address::new(base + 2))?;
                let attrs = self.voam.read_offset(Address::new(base + 3))?;
                if self.ly() as i16 >= sy
                    && (self.ly() as i16) < sy + sprite_size
                    && sprite_count < 10
                {
                    sprites[sprite_count] = Sprite {
                        x: sx,
                        y: sy,
                        tile,
                        attrs,
                        oam_index: i,
                    };
                    sprite_count += 1;
                }
            }

            if sprite_count > 0 {
                sprites[..sprite_count].sort_unstable_by(|a, b| {
                    if !cgb.enabled() && a.x != b.x {
                        return b.x.cmp(&a.x);
                    }
                    return b.oam_index.cmp(&a.oam_index);
                });

                for sprite in &sprites[..sprite_count] {
                    if sprite.x < -7 || sprite.x >= (Self::SCREEN_WIDTH as i16) {
                        continue;
                    }

                    let usepal1: bool = sprite.attrs & (1 << 4) != 0;
                    let xflip: bool = sprite.attrs & (1 << 5) != 0;
                    let yflip: bool = sprite.attrs & (1 << 6) != 0;
                    let belowbg: bool = sprite.attrs & (1 << 7) != 0;
                    let c_palnr = sprite.attrs & 0x07;
                    let c_vram1: bool = sprite.attrs & (1 << 3) != 0;

                    let tiley: u16 = if yflip {
                        (sprite_size - 1 - (self.ly() as i16 - sprite.y)) as u16
                    } else {
                        (self.ly() as i16 - sprite.y) as u16
                    };

                    let tile_address = Address::new(
                        sprite.tile as u16 * 16
                            + tiley * 2
                            + if c_vram1 && cgb.enabled() {
                                Self::VRAM_BANK_SIZE
                            } else {
                                0
                            },
                    );

                    let (b1, b2) = (
                        self.vram.read_offset(tile_address)?,
                        self.vram.read_offset(tile_address + 1)?,
                    );

                    'xloop: for x in 0..8 {
                        if sprite.x + x < 0 || sprite.x + x >= (Self::SCREEN_WIDTH as i16) {
                            continue;
                        }

                        let xbit = 1 << (if xflip { x } else { 7 - x } as u32);
                        let colnr = (if b1 & xbit != 0 { 1 } else { 0 })
                            | (if b2 & xbit != 0 { 2 } else { 0 });
                        if colnr == 0 {
                            continue;
                        }

                        if cgb.enabled() {
                            if self.lcdc & 0x01 != 0
                                && (bgprio[(sprite.x + x) as usize] == PrioType::PrioFlag
                                    || (belowbg
                                        && bgprio[(sprite.x + x) as usize] != PrioType::Color0))
                            {
                                continue 'xloop;
                            }
                            let c_palnr = c_palnr as usize;
                            let r = self.ocpd[c_palnr][colnr][0];
                            let g = self.ocpd[c_palnr][colnr][1];
                            let b = self.ocpd[c_palnr][colnr][2];
                            self.framebuffer[(sprite.x + x) as usize] = Pixel::rgb(r, g, b);
                        } else {
                            if belowbg && bgprio[(sprite.x + x) as usize] != PrioType::Color0 {
                                continue 'xloop;
                            }

                            self.framebuffer[(sprite.x + x) as usize] =
                                Pixel::monochrome(c_palnr, colnr as u8);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub const fn clock(&self) -> u16 {
        self.clock
    }

    pub const fn framebuffer(&self) -> &[Pixel] {
        &*self.framebuffer
    }

    pub const fn lcdc(&self) -> u8 {
        self.lcdc
    }

    pub const fn stat(&self) -> u8 {
        self.stat & (1 >> 7)
    }

    pub const fn mode(&self) -> u8 {
        self.stat & 0b11
    }
}
