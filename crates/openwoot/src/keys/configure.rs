use openwoot_protocol::{
    advanced, gamepad,
    keys::{self as config, Layer},
};

use super::{
    Entry, Functions, Key, Keymaps, Keys, Parts, Profile, RapidTriggerOverride, Switch, Tuning,
    travel_of,
};
use crate::{analog, lighting::TRAVEL_FULL};

impl<F: Functions, const ROWS: usize, const COLUMNS: usize> Keys<F, ROWS, COLUMNS> {
    pub fn configure(&mut self, profile: &Profile, parts: Parts) {
        if parts.contains(Parts::ACTIVATED) {
            self.reset_keys();
        }
        if parts.contains(Parts::SETTINGS) {
            self.configure_settings(&profile.settings);
        }
        if parts.contains(Parts::KEYMAPS) {
            self.configure_keymaps(&profile.keymaps);
        }
        if parts.contains(Parts::OVERRIDES) {
            self.configure_overrides(&profile.overrides);
        }
        if parts.contains(Parts::ADVANCED) {
            self.configure_advanced(&profile.advanced);
        }
        if parts.contains(Parts::GAMEPAD_MAPPING) {
            self.configure_gamepad_mapping(&profile.gamepad_mapping);
        }
        if parts.contains(Parts::GAMEPAD_SETTINGS) {
            self.set_gamepad_settings(Some(profile.gamepad_settings));
        }
    }

    pub fn configure_settings(&mut self, settings: &config::Settings) {
        let minimum = self.largest_minimum();
        self.set_tuning(Tuning::from_config(settings, minimum));
    }

    pub fn configure_keymaps(&mut self, keymaps: &Keymaps) {
        for (layer, keymap) in Layer::ALL.into_iter().zip(&keymaps.0) {
            for key in matrix::<ROWS, COLUMNS>() {
                let action = keymap.get(key.index(COLUMNS)).copied().unwrap_or_default();
                self.set_action(key, layer, action);
            }
        }
    }

    pub fn configure_overrides(&mut self, overrides: &config::Overrides) {
        for key in matrix::<ROWS, COLUMNS>() {
            let entry = overrides
                .get(key.index(COLUMNS))
                .copied()
                .unwrap_or_default();
            let minimum = self.minimum_or_default(key);
            self.set_override(
                key,
                entry.actuation.map(travel_of),
                RapidTriggerOverride::from_config(&entry.rapid_trigger, minimum),
            );
        }
    }

    pub fn configure_advanced(&mut self, entries: &advanced::Keys) {
        let converted: heapless::Vec<Entry, { advanced::MAX_ENTRIES }> = entries
            .iter()
            .filter_map(|entry| {
                Entry::from_config(entry, COLUMNS, |key| self.minimum_or_default(key))
            })
            .collect();
        self.set_advanced(converted);
    }

    pub fn configure_gamepad_mapping(&mut self, mapping: &gamepad::Mapping) {
        self.set_gamepad_exclusive(mapping.exclusive);
        for key in matrix::<ROWS, COLUMNS>() {
            let control = mapping.bindings.get(key.index(COLUMNS)).copied().flatten();
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
