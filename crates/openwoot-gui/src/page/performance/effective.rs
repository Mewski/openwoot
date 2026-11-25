use openwoot_protocol::keys::{Distance, Override, RapidTrigger, RapidTriggerOverride, Settings};

const TRAVEL_FULL: u16 = 4095;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Selected<T> {
    Nothing,
    Same(T),
    Mixed(T),
}

impl<T: PartialEq + Copy> Selected<T> {
    pub(super) fn of(values: impl IntoIterator<Item = T>) -> Self {
        let mut values = values.into_iter();
        match values.next() {
            None => Self::Nothing,
            Some(first) if values.all(|value| value == first) => Self::Same(first),
            Some(first) => Self::Mixed(first),
        }
    }
}

pub(super) fn actuation(settings: &Settings, key: &Override) -> Distance {
    key.actuation.unwrap_or(settings.actuation)
}

pub(super) const fn rapid_trigger(settings: &Settings, key: &Override) -> Option<RapidTrigger> {
    match key.rapid_trigger {
        RapidTriggerOverride::Profile if settings.rapid_trigger_enabled => {
            Some(settings.rapid_trigger)
        }
        RapidTriggerOverride::Profile | RapidTriggerOverride::Off => None,
        RapidTriggerOverride::On => Some(settings.rapid_trigger),
        RapidTriggerOverride::With(own) => Some(own),
    }
}

pub(super) const fn parameters(settings: &Settings, key: &Override) -> RapidTrigger {
    match key.rapid_trigger {
        RapidTriggerOverride::With(own) => own,
        RapidTriggerOverride::Profile | RapidTriggerOverride::Off | RapidTriggerOverride::On => {
            settings.rapid_trigger
        }
    }
}

pub(super) fn travel_fraction(travel: u16) -> f32 {
    f32::from(travel.min(TRAVEL_FULL)) / f32::from(TRAVEL_FULL)
}

pub(super) fn reaches(travel: u16, distance: Distance) -> bool {
    travel > 0
        && u32::from(travel) * u32::from(u16::MAX)
            >= u32::from(distance.fraction()) * u32::from(TRAVEL_FULL)
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::keys::{ScanMode, TapDuration};

    use super::*;

    const PROFILE_RAPID_TRIGGER: RapidTrigger = RapidTrigger {
        press: Distance::from_fraction(2000),
        release: None,
        continuous: false,
    };
    const OWN_RAPID_TRIGGER: RapidTrigger = RapidTrigger {
        press: Distance::from_fraction(500),
        release: Some(Distance::from_fraction(900)),
        continuous: true,
    };

    const fn settings(rapid_trigger_enabled: bool) -> Settings {
        Settings {
            actuation: Distance::from_fraction(20_000),
            rapid_trigger_enabled,
            rapid_trigger: PROFILE_RAPID_TRIGGER,
            tap_duration: TapDuration::FortyMilliseconds,
            scan_mode: ScanMode::Single,
            keyboard_in_gamepad_mode: false,
        }
    }

    const fn key(rapid_trigger: RapidTriggerOverride) -> Override {
        Override {
            actuation: None,
            rapid_trigger,
        }
    }

    #[test]
    fn a_key_without_overrides_follows_the_profile() {
        let entry = Override::default();
        assert_eq!(
            actuation(&settings(true), &entry),
            Distance::from_fraction(20_000)
        );
        assert_eq!(
            rapid_trigger(&settings(true), &entry),
            Some(PROFILE_RAPID_TRIGGER)
        );
        assert_eq!(rapid_trigger(&settings(false), &entry), None);
    }

    #[test]
    fn overrides_replace_the_actuation_point_and_the_rapid_trigger_choice() {
        let own = Override {
            actuation: Some(Distance::from_fraction(3000)),
            rapid_trigger: RapidTriggerOverride::Off,
        };
        assert_eq!(
            actuation(&settings(true), &own),
            Distance::from_fraction(3000)
        );
        assert_eq!(rapid_trigger(&settings(true), &own), None);
        assert_eq!(
            rapid_trigger(&settings(false), &key(RapidTriggerOverride::On)),
            Some(PROFILE_RAPID_TRIGGER)
        );
        assert_eq!(
            rapid_trigger(
                &settings(false),
                &key(RapidTriggerOverride::With(OWN_RAPID_TRIGGER))
            ),
            Some(OWN_RAPID_TRIGGER)
        );
        assert_eq!(
            parameters(&settings(false), &key(RapidTriggerOverride::On)),
            PROFILE_RAPID_TRIGGER
        );
    }

    #[test]
    fn selected_values_are_the_same_mixed_or_nothing() {
        assert_eq!(Selected::<u8>::of([]), Selected::Nothing);
        assert_eq!(Selected::of([3, 3, 3]), Selected::Same(3));
        assert_eq!(Selected::of([3, 4, 3]), Selected::Mixed(3));
        assert_eq!(Selected::of([1, 2]), Selected::Mixed(1));
    }

    #[test]
    fn travel_reaches_the_actuation_point_at_its_share_of_the_full_travel() {
        let half = Distance::from_fraction(u16::MAX / 2);
        assert!(!reaches(0, Distance::from_fraction(1)));
        assert!(!reaches(2046, half));
        assert!(reaches(2048, half));
        assert!(reaches(TRAVEL_FULL, Distance::FULL_TRAVEL));
        assert!((travel_fraction(TRAVEL_FULL) - 1.0).abs() < f32::EPSILON);
        assert!((travel_fraction(u16::MAX) - 1.0).abs() < f32::EPSILON);
        assert!(travel_fraction(0).abs() < f32::EPSILON);
    }
}
