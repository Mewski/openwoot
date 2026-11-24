use openwoot_protocol::keys::{Distance, Override, RapidTrigger, RapidTriggerOverride, Settings};

const TRAVEL_FULL: u16 = 4095;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RapidTriggerMode {
    Profile,
    Off,
    On,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Selected<T> {
    Nothing,
    Same(T),
    Mixed(T),
}

impl RapidTriggerMode {
    pub(super) const ALL: [Self; 4] = [Self::Profile, Self::Off, Self::On, Self::Custom];

    pub(super) const fn of(rapid_trigger: RapidTriggerOverride) -> Self {
        match rapid_trigger {
            RapidTriggerOverride::Profile => Self::Profile,
            RapidTriggerOverride::Off => Self::Off,
            RapidTriggerOverride::On => Self::On,
            RapidTriggerOverride::With(_) => Self::Custom,
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Profile => "Profile",
            Self::Off => "Off",
            Self::On => "On",
            Self::Custom => "Custom",
        }
    }

    pub(super) const fn description(self) -> &'static str {
        match self {
            Self::Profile => {
                "The keys follow the profile: Rapid Trigger is on when it is on for the profile."
            }
            Self::Off => "Rapid Trigger stays off for these keys, whatever the profile says.",
            Self::On => "Rapid Trigger is on for these keys with the sensitivity of the profile.",
            Self::Custom => "Rapid Trigger is on for these keys with their own sensitivity.",
        }
    }

    pub(super) const fn apply(
        self,
        current: RapidTriggerOverride,
        profile: RapidTrigger,
    ) -> RapidTriggerOverride {
        match (self, current) {
            (Self::Profile, _) => RapidTriggerOverride::Profile,
            (Self::Off, _) => RapidTriggerOverride::Off,
            (Self::On, _) => RapidTriggerOverride::On,
            (Self::Custom, RapidTriggerOverride::With(own)) => RapidTriggerOverride::With(own),
            (Self::Custom, _) => RapidTriggerOverride::With(profile),
        }
    }
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

    pub(super) const fn first(self) -> Option<T> {
        match self {
            Self::Nothing => None,
            Self::Same(first) | Self::Mixed(first) => Some(first),
        }
    }

    pub(super) const fn is_mixed(self) -> bool {
        matches!(self, Self::Mixed(_))
    }

    pub(super) const fn same(self) -> Option<T> {
        match self {
            Self::Same(value) => Some(value),
            Self::Nothing | Self::Mixed(_) => None,
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

pub(super) fn has_own_settings(key: &Override) -> bool {
    *key != Override::default()
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
        assert!(!has_own_settings(&entry));
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
        assert!(has_own_settings(&own));
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
    fn custom_rapid_trigger_starts_from_the_profile_and_keeps_own_parameters() {
        assert_eq!(
            RapidTriggerMode::Custom.apply(RapidTriggerOverride::Off, PROFILE_RAPID_TRIGGER),
            RapidTriggerOverride::With(PROFILE_RAPID_TRIGGER)
        );
        assert_eq!(
            RapidTriggerMode::Custom.apply(
                RapidTriggerOverride::With(OWN_RAPID_TRIGGER),
                PROFILE_RAPID_TRIGGER
            ),
            RapidTriggerOverride::With(OWN_RAPID_TRIGGER)
        );
        for mode in RapidTriggerMode::ALL {
            let applied = mode.apply(RapidTriggerOverride::On, PROFILE_RAPID_TRIGGER);
            assert_eq!(RapidTriggerMode::of(applied), mode);
        }
    }

    #[test]
    fn selected_values_are_the_same_mixed_or_nothing() {
        assert_eq!(Selected::<u8>::of([]), Selected::Nothing);
        assert_eq!(Selected::of([3, 3, 3]), Selected::Same(3));
        assert_eq!(Selected::of([3, 4, 3]), Selected::Mixed(3));
        assert!(Selected::of([1, 2]).is_mixed());
        assert_eq!(Selected::of([7, 7]).first(), Some(7));
        assert_eq!(Selected::of([7, 7]).same(), Some(7));
        assert_eq!(Selected::of([7, 8]).same(), None);
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
