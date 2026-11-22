use embassy_usb::driver;
use openwoot_protocol::device;

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

impl From<driver::Speed> for Speed {
    fn from(negotiated: driver::Speed) -> Self {
        match negotiated {
            driver::Speed::High => Self::High,
            driver::Speed::Full | driver::Speed::Low => Self::Full,
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

impl From<device::Settings> for Face {
    fn from(settings: device::Settings) -> Self {
        Self {
            mode: match settings.mode {
                device::Mode::Xinput => Mode::Xinput,
                device::Mode::Gamepad => Mode::Gamepad,
                device::Mode::NoGamepad => Mode::NoGamepad,
            },
            keyboard: match settings.keyboard {
                device::KeyboardFormat::NKeyRollover => KeyboardFormat::NKeyRollover,
                device::KeyboardFormat::Boot => KeyboardFormat::Boot,
            },
            speed: match settings.speed {
                device::Speed::Full => Speed::Full,
                device::Speed::High => Speed::High,
            },
            gamepad_polling: match settings.gamepad_polling {
                device::PollingRate::Every4Milliseconds => PollingRate::Every4Milliseconds,
                device::PollingRate::Every1Millisecond => PollingRate::Every1Millisecond,
                device::PollingRate::Every125Microseconds => PollingRate::Every125Microseconds,
            },
        }
    }
}

impl From<Face> for device::Settings {
    fn from(face: Face) -> Self {
        Self {
            mode: match face.mode {
                Mode::Xinput => device::Mode::Xinput,
                Mode::Gamepad => device::Mode::Gamepad,
                Mode::NoGamepad => device::Mode::NoGamepad,
            },
            keyboard: match face.keyboard {
                KeyboardFormat::NKeyRollover => device::KeyboardFormat::NKeyRollover,
                KeyboardFormat::Boot => device::KeyboardFormat::Boot,
            },
            speed: match face.speed {
                Speed::Full => device::Speed::Full,
                Speed::High => device::Speed::High,
            },
            gamepad_polling: match face.gamepad_polling {
                PollingRate::Every4Milliseconds => device::PollingRate::Every4Milliseconds,
                PollingRate::Every1Millisecond => device::PollingRate::Every1Millisecond,
                PollingRate::Every125Microseconds => device::PollingRate::Every125Microseconds,
            },
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_device_settings_give_the_default_face() {
        assert_eq!(Face::from(device::Settings::default()), Face::default());
    }

    #[test]
    fn every_setting_reaches_the_face() {
        let face = Face::from(device::Settings {
            mode: device::Mode::Xinput,
            keyboard: device::KeyboardFormat::Boot,
            speed: device::Speed::Full,
            gamepad_polling: device::PollingRate::Every125Microseconds,
        });
        assert_eq!(
            face,
            Face {
                mode: Mode::Xinput,
                keyboard: KeyboardFormat::Boot,
                speed: Speed::Full,
                gamepad_polling: PollingRate::Every125Microseconds,
            }
        );
    }

    #[test]
    fn a_face_converts_back_to_the_same_device_settings() {
        let settings = device::Settings {
            mode: device::Mode::Gamepad,
            keyboard: device::KeyboardFormat::Boot,
            speed: device::Speed::Full,
            gamepad_polling: device::PollingRate::Every4Milliseconds,
        };
        assert_eq!(device::Settings::from(Face::from(settings)), settings);
    }
}
