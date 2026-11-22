use openwoot_protocol::device;

use crate::usb::{Face, KeyboardFormat, Mode, PollingRate, Speed};

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
}
