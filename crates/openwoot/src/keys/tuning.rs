use openwoot_protocol::keys::{self as config, Distance, TapDuration};

use super::{RapidTrigger, travel_of};

pub(crate) const DEFAULT_MINIMUM: u16 = 112;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tuning {
    pub actuation_point: u16,
    pub rapid_trigger_enabled: bool,
    pub rapid_trigger: RapidTrigger,
    pub tap_ms: u8,
    pub keyboard_in_gamepad_mode: bool,
}

impl Default for Tuning {
    fn default() -> Self {
        Self::from_config(&default_settings(), DEFAULT_MINIMUM)
    }
}

impl Tuning {
    #[must_use]
    pub fn from_config(settings: &config::Settings, minimum: u16) -> Self {
        Self {
            actuation_point: travel_of(settings.actuation),
            rapid_trigger_enabled: settings.rapid_trigger_enabled,
            rapid_trigger: RapidTrigger::from_config(&settings.rapid_trigger, minimum),
            tap_ms: settings.tap_duration.milliseconds(),
            keyboard_in_gamepad_mode: settings.keyboard_in_gamepad_mode,
        }
    }
}

#[must_use]
pub const fn default_settings() -> config::Settings {
    config::Settings {
        actuation: Distance::from_fraction(63 * 257),
        rapid_trigger_enabled: false,
        rapid_trigger: config::RapidTrigger {
            press: Distance::from_fraction(26 * 257),
            release: None,
            continuous: false,
        },
        tap_duration: TapDuration::FortyMilliseconds,
        scan_mode: config::ScanMode::Parallel,
        keyboard_in_gamepad_mode: false,
    }
}
