pub(crate) const INTERFACE_CLASS: u8 = 0xFF;
pub(crate) const INTERFACE_SUBCLASS: u8 = 0x5D;
pub(crate) const INTERFACE_PROTOCOL: u8 = 0x01;
pub(crate) const CLASS_DESCRIPTOR_TYPE: u8 = 0x21;
pub(crate) const MAX_PACKET_SIZE: u16 = 32;
pub(crate) const MS_OS_STRING_INDEX: u8 = 0xEE;
pub(crate) const MS_OS_VENDOR_CODE: u8 = 0xB0;
pub(crate) const MS_OS_STRING: &str = "MSFT100\u{B0}";
pub(crate) const EXTENDED_COMPAT_ID_INDEX: u16 = 4;
pub(crate) const EXTENDED_COMPAT_ID: [u8; 40] = extended_compat_id(0, *b"XUSB10\0\0");

const LED_COMMAND: u8 = 0x01;
const LED_COMMAND_LEN: u8 = 0x03;
const REPORTED_INPUT_LEN: u8 = 0x14;
const REPORTED_OUTPUT_LEN: u8 = 0x08;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedPattern(u8);

#[must_use]
pub(crate) const fn class_descriptor(input_endpoint: u8, output_endpoint: u8) -> [u8; 15] {
    [
        0x00,
        0x01,
        0x01,
        0x25,
        input_endpoint,
        REPORTED_INPUT_LEN,
        0x00,
        0x00,
        0x00,
        0x00,
        0x13,
        output_endpoint,
        REPORTED_OUTPUT_LEN,
        0x00,
        0x00,
    ]
}

impl LedPattern {
    #[must_use]
    pub fn from_packet(packet: &[u8]) -> Option<Self> {
        match packet {
            [LED_COMMAND, LED_COMMAND_LEN, pattern, ..] => Some(Self(*pattern)),
            _ => None,
        }
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn player(self) -> Option<u8> {
        match self.0 {
            2..=5 => Some(self.0 - 2),
            6..=9 => Some(self.0 - 6),
            _ => None,
        }
    }
}

const fn extended_compat_id(interface: u8, compatible_id: [u8; 8]) -> [u8; 40] {
    let [length, ..] = 40u32.to_le_bytes();
    let [first, second, third, fourth, fifth, sixth, seventh, eighth] = compatible_id;
    [
        length, 0x00, 0x00, 0x00, 0x00, 0x01, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, interface, 0x01, first, second, third, fourth, fifth, sixth, seventh, eighth, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]
}
