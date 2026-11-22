use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    config::{Capabilities, Invalid, MAX_KEYS, Validate},
    gamepad::Control,
    profile::Slot,
};

pub const LAYERS: usize = 4;

pub type Keymap = Vec<Action, MAX_KEYS>;
pub type Overrides = Vec<Override, MAX_KEYS>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Schema)]
pub struct Index(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Position {
    pub row: u8,
    pub column: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Layer {
    #[default]
    Base,
    One,
    Two,
    Three,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Action {
    #[default]
    None,
    Keyboard(u8),
    Consumer(u16),
    System(SystemControl),
    Mouse(MouseButton),
    Gamepad(Control),
    Layer(Layer),
    Function(Function),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum SystemControl {
    PowerDown,
    Sleep,
    WakeUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Function {
    BrightnessUp,
    BrightnessDown,
    Profile(Slot),
    NextProfile,
    ToggleModeProfile,
    ToggleKeyDisable,
    LayerLock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Schema)]
pub struct Distance(u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct RapidTrigger {
    pub press: Distance,
    pub release: Option<Distance>,
    pub continuous: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum RapidTriggerOverride {
    #[default]
    Profile,
    Off,
    On,
    With(RapidTrigger),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Override {
    pub actuation: Option<Distance>,
    pub rapid_trigger: RapidTriggerOverride,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Settings {
    pub actuation: Distance,
    pub rapid_trigger_enabled: bool,
    pub rapid_trigger: RapidTrigger,
    pub tap_duration: TapDuration,
    pub scan_mode: ScanMode,
    pub keyboard_in_gamepad_mode: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum TapDuration {
    OneMillisecond,
    TwentyMilliseconds,
    #[default]
    FortyMilliseconds,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum ScanMode {
    Single,
    #[default]
    Parallel,
}

impl Index {
    #[must_use]
    pub const fn new(index: u8) -> Self {
        Self(index)
    }

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Layer {
    pub const ALL: [Self; LAYERS] = [Self::Base, Self::One, Self::Two, Self::Three];

    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Base => 0,
            Self::One => 1,
            Self::Two => 2,
            Self::Three => 3,
        }
    }
}

impl Action {
    pub const CONSUMER_USAGES: core::ops::RangeInclusive<u16> = 0x0001..=0x02A2;
    pub const KEYBOARD_USAGES: [core::ops::RangeInclusive<u8>; 2] = [0x04..=0xDD, 0xE0..=0xE7];

    pub fn validate(self, capabilities: &Capabilities) -> Result<(), Invalid> {
        match self {
            Self::Keyboard(usage)
                if !Self::KEYBOARD_USAGES
                    .iter()
                    .any(|usages| usages.contains(&usage)) =>
            {
                Err(Invalid::KeyboardUsage(usage))
            }
            Self::Consumer(usage) if !Self::CONSUMER_USAGES.contains(&usage) => {
                Err(Invalid::ConsumerUsage(usage))
            }
            Self::Layer(Layer::Base) => Err(Invalid::MomentaryBaseLayer),
            Self::Function(Function::Profile(profile)) => capabilities.check_profile(profile),
            _ => Ok(()),
        }
    }
}

impl Distance {
    pub const FULL_TRAVEL: Self = Self(u16::MAX);

    #[must_use]
    pub const fn from_fraction(fraction: u16) -> Self {
        Self(fraction)
    }

    #[must_use]
    pub const fn fraction(self) -> u16 {
        self.0
    }

    pub fn validate(self) -> Result<(), Invalid> {
        if self.0 == 0 {
            Err(Invalid::ZeroDistance)
        } else {
            Ok(())
        }
    }
}

impl RapidTrigger {
    pub fn validate(&self) -> Result<(), Invalid> {
        self.press.validate()?;
        self.release.map_or(Ok(()), Distance::validate)
    }
}

impl Override {
    pub fn validate(&self) -> Result<(), Invalid> {
        self.actuation.map_or(Ok(()), Distance::validate)?;
        match &self.rapid_trigger {
            RapidTriggerOverride::With(rapid_trigger) => rapid_trigger.validate(),
            RapidTriggerOverride::Profile
            | RapidTriggerOverride::Off
            | RapidTriggerOverride::On => Ok(()),
        }
    }
}

impl TapDuration {
    #[must_use]
    pub const fn milliseconds(self) -> u8 {
        match self {
            Self::OneMillisecond => 1,
            Self::TwentyMilliseconds => 20,
            Self::FortyMilliseconds => 40,
        }
    }
}

impl Validate for Keymap {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        capabilities.check_key_count(self.len())?;
        self.iter()
            .try_for_each(|action| action.validate(capabilities))
    }
}

impl Validate for Overrides {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        capabilities.check_key_count(self.len())?;
        self.iter().try_for_each(Override::validate)
    }
}

impl Validate for Settings {
    fn validate(&self, _: &Capabilities) -> Result<(), Invalid> {
        self.actuation.validate()?;
        self.rapid_trigger.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_and_report_usages_are_keyboard_actions_and_the_gap_is_not() {
        let capabilities = Capabilities {
            key_rows: 1,
            key_columns: 1,
            profiles: 1,
            layouts: Vec::new(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 0,
            edition: None,
        };
        for usage in [0x04, 0xDD, 0xE0, 0xE7] {
            assert_eq!(Action::Keyboard(usage).validate(&capabilities), Ok(()));
        }
        for usage in [0x00, 0x03, 0xDE, 0xDF, 0xE8, 0xFF] {
            assert_eq!(
                Action::Keyboard(usage).validate(&capabilities),
                Err(Invalid::KeyboardUsage(usage))
            );
        }
    }

    #[test]
    fn tap_durations_are_the_three_c_presets() {
        assert_eq!(
            [
                TapDuration::OneMillisecond,
                TapDuration::TwentyMilliseconds,
                TapDuration::FortyMilliseconds
            ]
            .map(TapDuration::milliseconds),
            [1, 20, 40]
        );
    }
}
