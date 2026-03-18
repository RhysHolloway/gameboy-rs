use std::borrow::Cow;

pub struct Cartridge<D: AsRef<[u8]>> {
    data: D,
    pub ram: Vec<u8>,
}

impl<D: AsRef<[u8]>> Cartridge<D> {

    pub fn new(data: D) -> Self {
        Self { data, ram: vec![0; 0x2000 * 64] }
    }

    pub fn title(&self) -> Cow<'_, str> {
        self.data.as_ref().get(0x134..0x144).map(String::from_utf8_lossy).unwrap_or(Cow::Borrowed("UNKNOWN"))
    }

    pub fn read(&self, address: usize) -> Option<u8> {
        self.data.as_ref().get(address).copied()
    }

}