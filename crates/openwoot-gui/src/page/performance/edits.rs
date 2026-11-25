use openwoot_protocol::keys::{Distance, Overrides, RapidTrigger, RapidTriggerOverride, Settings};

pub(super) fn set_actuation(
    settings: &Settings,
    overrides: &mut Overrides,
    key: usize,
    actuation: Distance,
) {
    if let Some(entry) = overrides.get_mut(key) {
        entry.actuation = (actuation != settings.actuation).then_some(actuation);
    }
}

pub(super) fn set_rapid_trigger(
    settings: &Settings,
    overrides: &mut Overrides,
    key: usize,
    rapid_trigger: Option<RapidTrigger>,
) {
    if let Some(entry) = overrides.get_mut(key) {
        entry.rapid_trigger = match rapid_trigger {
            None if settings.rapid_trigger_enabled => RapidTriggerOverride::Off,
            None => RapidTriggerOverride::Profile,
            Some(own) if own == settings.rapid_trigger && settings.rapid_trigger_enabled => {
                RapidTriggerOverride::Profile
            }
            Some(own) if own == settings.rapid_trigger => RapidTriggerOverride::On,
            Some(own) => RapidTriggerOverride::With(own),
        };
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::keys::{Override, ScanMode, TapDuration};

    use super::*;

    const PROFILE: RapidTrigger = RapidTrigger {
        press: Distance::from_fraction(2000),
        release: None,
        continuous: false,
    };
    const OWN: RapidTrigger = RapidTrigger {
        press: Distance::from_fraction(500),
        release: Some(Distance::from_fraction(900)),
        continuous: true,
    };

    const fn settings(rapid_trigger_enabled: bool) -> Settings {
        Settings {
            actuation: Distance::from_fraction(20_000),
            rapid_trigger_enabled,
            rapid_trigger: PROFILE,
            tap_duration: TapDuration::FortyMilliseconds,
            scan_mode: ScanMode::Single,
            keyboard_in_gamepad_mode: false,
        }
    }

    fn overrides(count: usize) -> Overrides {
        (0..count).map(|_| Override::default()).collect()
    }

    #[test]
    fn a_key_set_to_the_profile_value_needs_no_override() {
        let profile = settings(false);
        let mut keys = overrides(3);
        set_actuation(&profile, &mut keys, 1, Distance::from_fraction(9000));
        assert_eq!(keys[1].actuation, Some(Distance::from_fraction(9000)));
        set_actuation(&profile, &mut keys, 1, profile.actuation);
        assert_eq!(keys[1], Override::default());
        set_actuation(&profile, &mut keys, 7, Distance::from_fraction(9000));
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn rapid_trigger_follows_the_profile_whenever_it_can() {
        let enabled = settings(true);
        let disabled = settings(false);
        let mut keys = overrides(1);
        set_rapid_trigger(&enabled, &mut keys, 0, Some(PROFILE));
        assert_eq!(keys[0].rapid_trigger, RapidTriggerOverride::Profile);
        set_rapid_trigger(&enabled, &mut keys, 0, None);
        assert_eq!(keys[0].rapid_trigger, RapidTriggerOverride::Off);
        set_rapid_trigger(&disabled, &mut keys, 0, Some(PROFILE));
        assert_eq!(keys[0].rapid_trigger, RapidTriggerOverride::On);
        set_rapid_trigger(&disabled, &mut keys, 0, None);
        assert_eq!(keys[0].rapid_trigger, RapidTriggerOverride::Profile);
        set_rapid_trigger(&disabled, &mut keys, 0, Some(OWN));
        assert_eq!(keys[0].rapid_trigger, RapidTriggerOverride::With(OWN));
    }
}
