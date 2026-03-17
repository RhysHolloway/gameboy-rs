use super::address::Address;

pub struct Bios {
    data: [u8; 256],
    enabled: bool,
}

impl Bios {

    pub fn get(&self, Address(addr): Address) -> Option<&u8> {
        self.enabled.then(|| self.data.get(addr as usize)).flatten()
    }

}

impl From<[u8; 256]> for Bios {
    fn from(value: [u8; 256]) -> Self {
        Self { data: value, enabled: true }
    }
}