use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};

use crate::keys::Key;

pub const TRAVEL_FULL: u16 = 4095;
pub const KEY_EVENT_CAPACITY: usize = 32;

pub type KeyEvents = Channel<CriticalSectionRawMutex, KeyActivity, KEY_EVENT_CAPACITY>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyEvent {
    Pressed,
    Released,
    Actuated,
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyActivity {
    pub key: Key,
    pub event: KeyEvent,
    pub at_ms: u32,
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
