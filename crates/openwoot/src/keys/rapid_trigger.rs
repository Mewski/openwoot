use openwoot_protocol::keys as config;

use super::travel_of;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RapidTrigger {
    pub press: u16,
    pub release: u16,
    pub continuous: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RapidTriggerOverride {
    #[default]
    Profile,
    Off,
    On,
    With(RapidTrigger),
}

impl RapidTrigger {
    #[must_use]
    pub fn from_config(rapid_trigger: &config::RapidTrigger, minimum: u16) -> Self {
        let press = travel_of(rapid_trigger.press).max(minimum);
        Self {
            press,
            release: rapid_trigger
                .release
                .map_or(press, |release| travel_of(release).max(minimum)),
            continuous: rapid_trigger.continuous,
        }
    }
}

impl RapidTriggerOverride {
    #[must_use]
    pub fn from_config(rapid_trigger: &config::RapidTriggerOverride, minimum: u16) -> Self {
        match rapid_trigger {
            config::RapidTriggerOverride::Profile => Self::Profile,
            config::RapidTriggerOverride::Off => Self::Off,
            config::RapidTriggerOverride::On => Self::On,
            config::RapidTriggerOverride::With(rapid_trigger) => {
                Self::With(RapidTrigger::from_config(rapid_trigger, minimum))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::keys::Distance;

    use super::*;

    #[test]
    fn the_release_sensitivity_defaults_to_the_press_sensitivity_and_both_keep_the_minimum() {
        let config = config::RapidTrigger {
            press: Distance::from_fraction(1),
            release: None,
            continuous: true,
        };
        assert_eq!(
            RapidTrigger::from_config(&config, 112),
            RapidTrigger {
                press: 112,
                release: 112,
                continuous: true
            }
        );
        let separate = config::RapidTrigger {
            press: Distance::from_fraction(0x8000),
            release: Some(Distance::from_fraction(0x4000)),
            continuous: false,
        };
        assert_eq!(
            RapidTrigger::from_config(&separate, 112),
            RapidTrigger {
                press: 2047,
                release: 1023,
                continuous: false
            }
        );
    }
}
