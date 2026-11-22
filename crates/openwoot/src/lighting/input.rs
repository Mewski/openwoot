use super::leds::Key;

pub const TRAVEL_FULL: u16 = 4095;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyEvent {
    Pressed,
    Released,
    Actuated,
    Reset,
}

pub trait KeyTravel {
    fn travel(&self, key: Key) -> u16 {
        let _ = key;
        0
    }

    fn gamepad_mode(&self) -> bool {
        false
    }

    fn response_curve(&self, travel: u16) -> f32 {
        f32::from(travel) / f32::from(TRAVEL_FULL)
    }
}

impl KeyTravel for () {}
