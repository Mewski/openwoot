use embedded_storage_async::nor_flash::MultiwriteNorFlash;
use openwoot_protocol::device;

use crate::{
    storage::{Scope, Settings},
    usb::{Face, Speed},
};

pub async fn stored_face<F: MultiwriteNorFlash>(
    settings: Option<&mut Settings<F>>,
    factory: Face,
) -> Face {
    let stored = match settings {
        Some(settings) => settings
            .load::<device::Settings>(Scope::Global)
            .await
            .ok()
            .flatten(),
        None => None,
    };
    face_of(stored, factory)
}

fn face_of(stored: Option<device::Settings>, factory: Face) -> Face {
    let Some(stored) = stored else {
        return factory;
    };
    let face = Face::from(stored);
    Face {
        speed: match face.speed {
            Speed::Full => Speed::Full,
            Speed::High => factory.speed,
        },
        ..face
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usb::{KeyboardFormat, Mode, PollingRate};

    const FULL_SPEED_BOARD: Face = Face {
        mode: Mode::NoGamepad,
        keyboard: KeyboardFormat::NKeyRollover,
        speed: Speed::Full,
        gamepad_polling: PollingRate::Every1Millisecond,
    };

    #[test]
    fn stored_settings_choose_the_face_but_never_raise_the_speed() {
        assert_eq!(face_of(None, FULL_SPEED_BOARD), FULL_SPEED_BOARD);
        let stored = device::Settings {
            mode: device::Mode::Xinput,
            keyboard: device::KeyboardFormat::Boot,
            speed: device::Speed::High,
            gamepad_polling: device::PollingRate::Every4Milliseconds,
        };
        assert_eq!(
            face_of(Some(stored), FULL_SPEED_BOARD),
            Face {
                mode: Mode::Xinput,
                keyboard: KeyboardFormat::Boot,
                speed: Speed::Full,
                gamepad_polling: PollingRate::Every4Milliseconds,
            }
        );
        let high_speed_board = Face {
            speed: Speed::High,
            ..FULL_SPEED_BOARD
        };
        let forced_full = device::Settings {
            speed: device::Speed::Full,
            ..stored
        };
        assert_eq!(
            face_of(Some(forced_full), high_speed_board).speed,
            Speed::Full
        );
        assert_eq!(face_of(Some(stored), high_speed_board).speed, Speed::High);
    }
}
