pub(super) const fn low_8(value: u32) -> u8 {
    value.to_le_bytes()[0]
}

pub(super) const fn low_16(value: u32) -> u16 {
    let [low, high, _, _] = value.to_le_bytes();
    u16::from_le_bytes([low, high])
}
