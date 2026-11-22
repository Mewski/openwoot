#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    Xinput,
    Gamepad,
    #[default]
    NoGamepad,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum KeyboardFormat {
    #[default]
    NKeyRollover,
    Boot,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Speed {
    Full,
    #[default]
    High,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PollingRate {
    Every4Milliseconds,
    #[default]
    Every1Millisecond,
    Every125Microseconds,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Face {
    pub mode: Mode,
    pub keyboard: KeyboardFormat,
    pub speed: Speed,
    pub gamepad_polling: PollingRate,
}

impl Mode {
    #[must_use]
    pub const fn product_id_offset(self) -> u16 {
        match self {
            Self::Xinput => 0,
            Self::Gamepad => 1,
            Self::NoGamepad => 2,
        }
    }
}

impl KeyboardFormat {
    #[must_use]
    pub const fn device_release(self) -> u16 {
        match self {
            Self::NKeyRollover => 0x0250,
            Self::Boot => 0x0251,
        }
    }
}

impl Face {
    #[must_use]
    pub const fn gamepad_interval(self) -> u8 {
        match (self.speed, self.gamepad_polling) {
            (Speed::High, PollingRate::Every4Milliseconds) => 6,
            (Speed::High, PollingRate::Every1Millisecond)
            | (Speed::Full, PollingRate::Every4Milliseconds) => 4,
            (Speed::High, PollingRate::Every125Microseconds)
            | (Speed::Full, PollingRate::Every1Millisecond | PollingRate::Every125Microseconds) => {
                1
            }
        }
    }
}
