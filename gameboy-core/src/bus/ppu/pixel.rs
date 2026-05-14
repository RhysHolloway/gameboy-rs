#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Pixel {
    pub rgb: [u8; 3],
}

impl Pixel {
    pub const fn monochrome(palette: u8, color: u8) -> Self {
        Self {
            rgb: match (palette >> (color & 0x03) * 2) & 0x03 {
                0 => [0xE0, 0xF8, 0xD0],
                1 => [0x88, 0xC0, 0x70],
                2 => [0x34, 0x68, 0x56],
                3 => [0x08, 0x18, 0x20],
                _ => unreachable!(),
            },
        }
    }

    pub const fn rgb(rgb: &[u8; 3]) -> Self {
        let r = rgb[0] as u16;
        let g = rgb[1] as u16;
        let b = rgb[2] as u16;
        Self {
            rgb: [
                ((r * 13 + g * 2 + b) >> 1) as u8,
                ((g * 3 + b) << 1) as u8,
                ((r * 3 + g * 2 + b * 11) >> 1) as u8,
            ],
        }
    }
}

impl std::ops::Deref for Pixel {
    type Target = [u8; 3];

    fn deref(&self) -> &Self::Target {
        &self.rgb
    }
}
