use openwoot_protocol::{advanced, keys::Layer};

use super::{
    Entry, Functions, Key, Keys, Parts, ProfileSource, RapidTriggerOverride, Switch, Tuning,
    travel_of,
};
use crate::{analog, lighting::TRAVEL_FULL};

impl<F: Functions, const ROWS: usize, const COLUMNS: usize> Keys<F, ROWS, COLUMNS> {
    pub fn configure(&mut self, profile: &impl ProfileSource, parts: Parts) {
        if parts.contains(Parts::ACTIVATED) {
            self.reset_keys();
        }
        if parts.contains(Parts::SETTINGS) {
            self.configure_settings(profile);
        }
        if parts.contains(Parts::KEYMAPS) {
            self.configure_keymaps(profile);
        }
        if parts.contains(Parts::OVERRIDES) {
            self.configure_overrides(profile);
        }
        if parts.contains(Parts::ADVANCED) {
            self.configure_advanced(profile);
        }
        if parts.contains(Parts::GAMEPAD_MAPPING) {
            self.configure_gamepad_mapping(profile);
        }
        if parts.contains(Parts::GAMEPAD_SETTINGS)
            && let Some(settings) = profile.read(|profile| profile.gamepad_settings)
        {
            self.set_gamepad_settings(Some(settings));
        }
    }

    fn configure_settings(&mut self, profile: &impl ProfileSource) {
        if let Some(settings) = profile.read(|profile| profile.settings) {
            let minimum = self.largest_minimum();
            self.set_tuning(Tuning::from_config(&settings, minimum));
        }
    }

    fn configure_keymaps(&mut self, profile: &impl ProfileSource) {
        for (index, layer) in Layer::ALL.into_iter().enumerate() {
            for key in matrix::<ROWS, COLUMNS>() {
                let Some(action) = profile.read(|profile| {
                    profile
                        .keymaps
                        .0
                        .get(index)
                        .and_then(|keymap| keymap.get(key.index(COLUMNS)).copied())
                }) else {
                    return;
                };
                self.set_action(key, layer, action.unwrap_or_default());
            }
        }
    }

    fn configure_overrides(&mut self, profile: &impl ProfileSource) {
        for key in matrix::<ROWS, COLUMNS>() {
            let Some(entry) =
                profile.read(|profile| profile.overrides.get(key.index(COLUMNS)).copied())
            else {
                return;
            };
            let entry = entry.unwrap_or_default();
            let minimum = self.minimum_or_default(key);
            self.set_override(
                key,
                entry.actuation.map(travel_of),
                RapidTriggerOverride::from_config(&entry.rapid_trigger, minimum),
            );
        }
    }

    fn configure_advanced(&mut self, profile: &impl ProfileSource) {
        let mut converted = heapless::Vec::<Entry, { advanced::MAX_ENTRIES }>::new();
        for position in 0..advanced::MAX_ENTRIES {
            let Some(entry) = profile.read(|profile| profile.advanced.get(position).copied())
            else {
                return;
            };
            let Some(entry) = entry else {
                break;
            };
            if let Some(entry) =
                Entry::from_config(&entry, COLUMNS, |key| self.minimum_or_default(key))
                && converted.push(entry).is_err()
            {
                break;
            }
        }
        self.set_advanced(converted);
    }

    fn configure_gamepad_mapping(&mut self, profile: &impl ProfileSource) {
        let Some(exclusive) = profile.read(|profile| profile.gamepad_mapping.exclusive) else {
            return;
        };
        self.set_gamepad_exclusive(exclusive);
        for key in matrix::<ROWS, COLUMNS>() {
            let Some(control) = profile.read(|profile| {
                profile
                    .gamepad_mapping
                    .bindings
                    .get(key.index(COLUMNS))
                    .copied()
                    .flatten()
            }) else {
                return;
            };
            self.set_gamepad_binding(key, control);
        }
    }

    fn largest_minimum(&self) -> u16 {
        matrix::<ROWS, COLUMNS>()
            .filter_map(|key| self.minimum(key))
            .max()
            .unwrap_or(super::DEFAULT_MINIMUM)
    }

    fn minimum_or_default(&self, key: Key) -> u16 {
        self.minimum(key).unwrap_or(super::DEFAULT_MINIMUM)
    }
}

impl Switch {
    #[must_use]
    pub fn from_switch(switch: &analog::Switch) -> Self {
        let remaining = u32::from(
            switch
                .kind()
                .travel()
                .saturating_sub(switch.travel_offset().unwrap_or_default()),
        );
        let last_step = remaining.saturating_sub(1);
        Self {
            minimum: u16::try_from(switch.travel_per_tenth_millimetre()).unwrap_or(u16::MAX),
            travel_limit: u16::try_from(
                (u32::from(TRAVEL_FULL) * last_step)
                    .checked_div(remaining)
                    .unwrap_or_default(),
            )
            .unwrap_or(TRAVEL_FULL),
        }
    }
}

fn matrix<const ROWS: usize, const COLUMNS: usize>() -> impl Iterator<Item = Key> {
    (0..ROWS).flat_map(|row| {
        (0..COLUMNS).filter_map(move |column| {
            Some(Key::new(
                u8::try_from(row).ok()?,
                u8::try_from(column).ok()?,
            ))
        })
    })
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;

    use openwoot_protocol::{
        advanced::Behaviour,
        gamepad::Control,
        keys::{Action, Distance, Index, Override},
    };

    use super::*;
    use crate::{
        keys::{DEFAULT_MINIMUM, Profile},
        layout::LAYERS,
    };

    const ROWS: usize = 2;
    const COLUMNS: usize = 3;
    const KEYS: usize = ROWS * COLUMNS;

    struct Counted {
        profile: Profile,
        reads: Cell<usize>,
        largest: Cell<usize>,
    }

    impl ProfileSource for Counted {
        fn read<R>(&self, read: impl FnOnce(&Profile) -> R) -> Option<R> {
            self.reads.set(self.reads.get() + 1);
            self.largest.set(self.largest.get().max(size_of::<R>()));
            Some(read(&self.profile))
        }
    }

    fn full_profile() -> Profile {
        let mut profile = Profile::empty();
        for (layer, keymap) in (0u8..).zip(profile.keymaps.0.iter_mut()) {
            keymap.extend(
                (0u8..)
                    .take(KEYS)
                    .map(|key| Action::Keyboard(layer * 16 + key)),
            );
        }
        profile.overrides.extend(core::iter::repeat_n(
            Override {
                actuation: Some(Distance::from_fraction(0x8000)),
                ..Override::default()
            },
            KEYS,
        ));
        profile.advanced.extend((0u8..2).map(|key| advanced::Key {
            key: Index::new(key),
            layer: Layer::Base,
            behaviour: Behaviour::Toggle(Action::Keyboard(0x04)),
        }));
        profile
            .gamepad_mapping
            .bindings
            .extend(core::iter::repeat_n(Some(Control::ButtonA), KEYS));
        profile
    }

    #[test]
    fn configuring_keys_copies_at_most_one_entry_per_critical_section() {
        let source = Counted {
            profile: full_profile(),
            reads: Cell::new(0),
            largest: Cell::new(0),
        };
        let mut keys = Keys::<(), ROWS, COLUMNS>::new(());
        for key in matrix::<ROWS, COLUMNS>() {
            keys.set_switch(
                key,
                Switch {
                    minimum: DEFAULT_MINIMUM,
                    travel_limit: TRAVEL_FULL,
                },
            );
        }
        keys.configure(&source, Parts::ALL.union(Parts::ACTIVATED));

        let entries = LAYERS * KEYS + KEYS + source.profile.advanced.len() + KEYS;
        assert!(source.reads.get() >= entries);
        assert!(source.largest.get() <= size_of::<Option<advanced::Key>>());
        assert_eq!(
            keys.action(Key::new(1, 2), Layer::Three),
            Action::Keyboard(0x35)
        );
        assert_eq!(
            keys.gamepad_settings(),
            Some(&source.profile.gamepad_settings)
        );
    }
}
