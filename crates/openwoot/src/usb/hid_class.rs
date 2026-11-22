pub(super) const CLASS: u8 = 0x03;
pub(super) const BOOT_SUBCLASS: u8 = 0x01;
pub(super) const KEYBOARD_PROTOCOL: u8 = 0x01;
pub(super) const DESCRIPTOR_TYPE: u8 = 0x21;
pub(super) const REPORT_DESCRIPTOR_TYPE: u8 = 0x22;

const DESCRIPTOR_LEN: u8 = 9;

pub(super) fn descriptor_body(report_descriptor: &[u8]) -> [u8; 7] {
    let [length_low, length_high] = u16::try_from(report_descriptor.len())
        .unwrap_or(u16::MAX)
        .to_le_bytes();
    [
        0x11,
        0x01,
        0x00,
        0x01,
        REPORT_DESCRIPTOR_TYPE,
        length_low,
        length_high,
    ]
}

pub(super) fn descriptor(report_descriptor: &[u8]) -> [u8; DESCRIPTOR_LEN as usize] {
    let mut descriptor = [0; DESCRIPTOR_LEN as usize];
    if let Some((head, body)) = descriptor.split_first_chunk_mut::<2>() {
        *head = [DESCRIPTOR_LEN, DESCRIPTOR_TYPE];
        body.copy_from_slice(&descriptor_body(report_descriptor));
    }
    descriptor
}
