use crate::hid::InputReport;

pub const REPORT_LEN: usize = REPORTED_LEN as usize;

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

const MESSAGE_TYPE: u8 = 0x00;
const REPORTED_LEN: u8 = 20;
const HEADER_LEN: usize = 2;
const LED_COMMAND: u8 = 0x01;
const LED_COMMAND_LEN: u8 = 0x03;
const REPORTED_INPUT_LEN: u8 = 0x14;
const REPORTED_OUTPUT_LEN: u8 = 0x08;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Buttons(u16);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stick {
    pub x: i16,
    pub y: i16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    pub buttons: Buttons,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub left_stick: Stick,
    pub right_stick: Stick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedPattern(u8);

impl Buttons {
    pub const BACK: Self = Self(0x0020);
    pub const BUTTON_A: Self = Self(0x1000);
    pub const BUTTON_B: Self = Self(0x2000);
    pub const BUTTON_X: Self = Self(0x4000);
    pub const BUTTON_Y: Self = Self(0x8000);
    pub const DPAD_DOWN: Self = Self(0x0002);
    pub const DPAD_LEFT: Self = Self(0x0004);
    pub const DPAD_RIGHT: Self = Self(0x0008);
    pub const DPAD_UP: Self = Self(0x0001);
    pub const GUIDE: Self = Self(0x0400);
    pub const LEFT_SHOULDER: Self = Self(0x0100);
    pub const LEFT_THUMB: Self = Self(0x0040);
    pub const RIGHT_SHOULDER: Self = Self(0x0200);
    pub const RIGHT_THUMB: Self = Self(0x0080);
    pub const START: Self = Self(0x0010);

    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl InputReport for Report {
    type Bytes = [u8; REPORT_LEN];

    const ID: Option<u8> = None;
    const LEN: usize = REPORT_LEN;

    fn to_bytes(&self) -> Self::Bytes {
        let words = [
            self.buttons.bits(),
            u16::from_le_bytes([self.left_trigger, self.right_trigger]),
            self.left_stick.x.cast_unsigned(),
            self.left_stick.y.cast_unsigned(),
            self.right_stick.x.cast_unsigned(),
            self.right_stick.y.cast_unsigned(),
        ];
        let mut bytes = [0; REPORT_LEN];
        if let Some((header, body)) = bytes.split_first_chunk_mut::<HEADER_LEN>() {
            *header = [MESSAGE_TYPE, REPORTED_LEN];
            for (chunk, word) in body.as_chunks_mut::<2>().0.iter_mut().zip(words) {
                *chunk = word.to_le_bytes();
            }
        }
        bytes
    }
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

const fn extended_compat_id(interface: u8, compatible_id: [u8; 8]) -> [u8; 40] {
    let [length, ..] = 40u32.to_le_bytes();
    let [first, second, third, fourth, fifth, sixth, seventh, eighth] = compatible_id;
    [
        length, 0x00, 0x00, 0x00, 0x00, 0x01, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, interface, 0x01, first, second, third, fourth, fifth, sixth, seventh, eighth, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_is_the_wired_controller_layout() {
        let report = Report {
            buttons: Buttons::BUTTON_A.union(Buttons::DPAD_UP),
            left_trigger: 0x11,
            right_trigger: 0x22,
            left_stick: Stick { x: -2, y: 0x0304 },
            right_stick: Stick {
                x: 0x0506,
                y: -0x8000,
            },
        };
        assert_eq!(
            report.to_bytes(),
            [
                0x00, 0x14, 0x01, 0x10, 0x11, 0x22, 0xFE, 0xFF, 0x04, 0x03, 0x06, 0x05, 0x00, 0x80,
                0, 0, 0, 0, 0, 0
            ]
        );
        assert!(report.buttons.contains(Buttons::BUTTON_A));
    }

    #[test]
    fn the_led_command_selects_the_player() {
        let player = |packet: &[u8]| LedPattern::from_packet(packet).and_then(LedPattern::player);
        assert_eq!(player(&[0x01, 0x03, 0x02]), Some(0));
        assert_eq!(player(&[0x01, 0x03, 0x09]), Some(3));
        assert_eq!(player(&[0x01, 0x03, 0x01]), None);
        assert_eq!(LedPattern::from_packet(&[0x00, 0x08, 0x00]), None);
        assert_eq!(
            LedPattern::from_packet(&[0x01, 0x03, 0x0A]).map(LedPattern::get),
            Some(0x0A)
        );
    }
}
