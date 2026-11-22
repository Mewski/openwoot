use super::InputReport;

pub const LEN: usize = REPORT_LEN as usize;

const MESSAGE_TYPE: u8 = 0x00;
const REPORT_LEN: u8 = 20;
const HEADER_LEN: usize = 2;

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
    type Bytes = [u8; LEN];

    const ID: Option<u8> = None;
    const LEN: usize = LEN;

    fn to_bytes(&self) -> Self::Bytes {
        let words = [
            self.buttons.bits(),
            u16::from_le_bytes([self.left_trigger, self.right_trigger]),
            self.left_stick.x.cast_unsigned(),
            self.left_stick.y.cast_unsigned(),
            self.right_stick.x.cast_unsigned(),
            self.right_stick.y.cast_unsigned(),
        ];
        let mut bytes = [0; LEN];
        if let Some((header, body)) = bytes.split_first_chunk_mut::<HEADER_LEN>() {
            *header = [MESSAGE_TYPE, REPORT_LEN];
            for (chunk, word) in body.as_chunks_mut::<2>().0.iter_mut().zip(words) {
                *chunk = word.to_le_bytes();
            }
        }
        bytes
    }
}
