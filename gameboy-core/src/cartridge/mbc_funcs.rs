pub(super) const fn ram_banks(v: u8) -> usize {
    match v {
        1 =>
        // "Listed in various unofficial docs as 2 KiB. However, a 2 KiB RAM chip was never
        // used in a cartridge. The source of this value is unknown."
        // Needed by some test roms. As we only deal in whole banks, just make it 1 8KiB bank.
        {
            1
        }
        2 => 1,
        3 => 4,
        4 => 16,
        5 => 8,
        _ => 0,
    }
}

pub(super) const fn rom_banks(v: u8) -> usize {
    if v <= 8 {
        2 << v
    } else {
        0
    }
}