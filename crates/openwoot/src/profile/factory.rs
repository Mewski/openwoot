use heapless::String;
use openwoot_protocol::{
    advanced, global,
    keys::{self, Distance, Index, Override, Overrides},
    lightbar,
    lighting::{KeyColors, Rgb, Settings},
    profile::{Info, NAME_LEN},
};

use super::{Appearance, Colors};
use crate::{
    layout::{DefaultKeymap, LAYERS},
    lighting::lightbar_profile,
    usb::Face,
};

#[derive(Debug)]
pub struct Factory<const ROWS: usize, const COLUMNS: usize> {
    pub keymap: DefaultKeymap<ROWS, COLUMNS>,
    pub presets: &'static [Preset],
    pub fallback: Preset,
    pub created: u8,
    pub global: global::Settings,
    pub lightbar: lightbar::Entries,
    pub face: Face,
}

#[derive(Debug)]
pub struct Preset {
    pub name: &'static str,
    pub color: Rgb,
    pub keys: keys::Settings,
    pub actuation_points: &'static [(Index, Distance)],
    pub advanced: &'static [advanced::Key],
    pub lighting: Settings,
    pub colors: [Option<&'static KeyColors>; LAYERS],
    pub lightbar_brightness: Option<u8>,
}

impl<const ROWS: usize, const COLUMNS: usize> Factory<ROWS, COLUMNS> {
    #[must_use]
    pub fn preset(&self, slot: u8) -> &Preset {
        self.presets
            .get(usize::from(slot))
            .unwrap_or(&self.fallback)
    }
}

impl Preset {
    #[must_use]
    pub fn info(&self) -> Info {
        let mut name = String::<NAME_LEN>::new();
        for character in self.name.chars() {
            if name.push(character).is_err() {
                break;
            }
        }
        Info {
            name,
            color: self.color,
        }
    }

    #[must_use]
    pub fn appearance(&self) -> Appearance {
        Appearance {
            settings: self.lighting.clone(),
            colors: Colors(self.colors.map(Option::<&KeyColors>::cloned)),
            lightbar: self.lightbar(),
        }
    }

    #[must_use]
    pub const fn lightbar(&self) -> lightbar::Settings {
        lightbar_profile(self.lightbar_brightness)
    }

    #[must_use]
    pub fn overrides(&self, keys: usize) -> Overrides {
        let mut overrides: Overrides = core::iter::repeat_n(Override::default(), keys).collect();
        for (key, actuation) in self.actuation_points {
            if let Some(entry) = overrides.get_mut(usize::from(key.get())) {
                entry.actuation = Some(*actuation);
            }
        }
        overrides
    }

    #[must_use]
    pub fn advanced_keys(&self) -> advanced::Keys {
        self.advanced.iter().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        heapless::Vec,
        lighting::{Color, Indicators},
    };

    use super::*;
    use crate::{
        keys::default_settings,
        layout::{DefaultKeymap, LAYERS},
    };

    const WHITE: Color = Color {
        red: 255,
        green: 255,
        blue: 255,
        alpha: 255,
    };
    const BLUE: Rgb = Rgb {
        red: 0,
        green: 0,
        blue: 255,
    };
    static BASE: KeyColors = Vec::from_array([BLUE; 2]);

    static PRESETS: [Preset; 1] = [preset(
        "Typing Profile",
        255,
        &[(Index::new(1), Distance::from_fraction(26 * 257))],
        Some(127),
    )];

    const fn preset(
        name: &'static str,
        brightness: u8,
        actuation_points: &'static [(Index, Distance)],
        lightbar_brightness: Option<u8>,
    ) -> Preset {
        Preset {
            name,
            color: BLUE,
            keys: default_settings(),
            actuation_points,
            advanced: &[],
            lighting: Settings {
                brightness,
                indicators: Indicators {
                    caps_lock: WHITE,
                    scroll_lock: WHITE,
                    num_lock: WHITE,
                    layer_lock: WHITE,
                },
                effects: Vec::new(),
            },
            colors: [Some(&BASE), None, None, None],
            lightbar_brightness,
        }
    }

    #[test]
    fn slots_past_the_presets_take_the_fallback() {
        let factory = Factory::<1, 2> {
            keymap: DefaultKeymap {
                layers: &[],
                variant_patches: &[],
            },
            presets: &PRESETS,
            fallback: preset("", 10, &[], None),
            created: 1,
            global: global::Settings {
                idle: None,
                num_lock_inverted: false,
                release_rebound_filter: false,
            },
            lightbar: lightbar::Entries::new(),
            face: Face::default(),
        };
        assert_eq!(factory.preset(0).info().name.as_str(), "Typing Profile");
        assert_eq!(factory.preset(3).lighting.brightness, 10);
        let appearance = factory.preset(0).appearance();
        assert_eq!(appearance.colors.0.len(), LAYERS);
        assert_eq!(appearance.colors.0[0].as_ref(), Some(&BASE));
        assert_eq!(appearance.colors.0[1], None);
        assert_eq!(appearance.lightbar.brightness, Some(127));
        assert!(appearance.lightbar.enabled && appearance.lightbar.use_global);
        assert_eq!(factory.preset(3).lightbar().brightness, None);
        let overrides = factory.preset(0).overrides(2);
        assert_eq!(overrides[0].actuation, None);
        assert_eq!(
            overrides[1].actuation,
            Some(Distance::from_fraction(26 * 257))
        );
        assert!(
            factory
                .preset(3)
                .overrides(2)
                .iter()
                .all(|entry| entry.actuation.is_none())
        );
    }

    #[test]
    fn a_long_name_is_cut_to_the_name_capacity() {
        let long = preset(
            "a name that is much longer than thirty-two bytes",
            0,
            &[],
            None,
        );
        assert_eq!(long.info().name.len(), NAME_LEN);
    }
}
