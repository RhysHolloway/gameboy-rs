#[derive(Debug, Clone, Default)]
pub struct Ir {
    pub i: u8,
}

impl Ir {

    pub const fn read(&self) -> u8 {
        self.i
    }

    pub const fn write(&mut self, value: u8) {
        self.i = value & (1 << 7) | 1;
    }
}