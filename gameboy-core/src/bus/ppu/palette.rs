use crate::Width;

#[derive(Default)]
pub struct Palette {
    index: u8,
    data: [[[u8; 3]; 4]; 8],
}

impl Palette {
    pub const fn read(&self, offset: Width) -> u8 {
        match offset {
            0 => 0x40 | self.index,
            1 => {
                let (select, palette, col) = self.index();
                let palette = &self.data[palette][col];
                if select {
                    palette[0] | ((palette[1] & 0x07) << 5)
                } else {
                    ((palette[1] & 0x18) >> 3) | (palette[2] << 2)
                }
            }
            _ => unreachable!(),
        }
    }

    pub const fn write(&mut self, offset: Width, value: u8) {
        match offset {
            0 => self.index = value & 0xBF,
            1 => {
                let (lower, palette, col) = self.index();
                let palette = &mut self.data[palette][col];
                if lower {
                    palette[0] = value & 0x1F;
                    palette[1] = (palette[1] & 0x18) | (value >> 5);
                } else {
                    palette[1] = (palette[1] & 0x07) | ((value & 0x3) << 3);
                    palette[2] = (value >> 2) & 0x1F;
                }
                if self.index & 0x80 != 0 {
                    self.index = 0x80 | ((self.index + 1) & 0x3F);
                }
            }
            _ => unreachable!(),
        }
    }

    const fn index(&self) -> (bool, usize, usize) {
        let select = self.index & 0x3F;
        let palette = (select >> 3) as usize;
        let col = ((select >> 1) & 0x3) as usize;
        (select & 0x01 == 0, palette, col)
    }

    pub const fn color(&self, palnr: u8, col: u8) -> &[u8; 3] {
        &self.data[palnr as usize][col as usize]
    }
}
