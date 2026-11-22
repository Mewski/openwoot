use heapless::String;
use openwoot_protocol::{
    global, keys,
    lighting::{KeyColors, Rgb, Settings},
    profile::{Info, NAME_LEN},
};

use super::{Appearance, Colors};
use crate::{
    layout::{DefaultKeymap, LAYERS},
    usb::Face,
};

#[derive(Debug)]
pub struct Factory<const ROWS: usize, const COLUMNS: usize> {
    pub keymap: DefaultKeymap<ROWS, COLUMNS>,
    pub presets: &'static [Preset],
    pub fallback: Preset,
    pub created: u8,
    pub global: global::Settings,
    pub face: Face,
}

#[derive(Debug)]
pub struct Preset {
    pub name: &'static str,
    pub color: Rgb,
    pub keys: keys::Settings,
    pub lighting: Settings,
    pub colors: [Option<&'static KeyColors>; LAYERS],
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
        }
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

    static PRESETS: [Preset; 1] = [preset("Typing Profile", 255)];

    const fn preset(name: &'static str, brightness: u8) -> Preset {
        Preset {
            name,
            color: BLUE,
            keys: default_settings(),
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
            fallback: preset("", 10),
            created: 1,
            global: global::Settings {
                idle: None,
                num_lock_inverted: false,
                release_rebound_filter: false,
            },
            face: Face::default(),
        };
        assert_eq!(factory.preset(0).info().name.as_str(), "Typing Profile");
        assert_eq!(factory.preset(3).lighting.brightness, 10);
        let appearance = factory.preset(0).appearance();
        assert_eq!(appearance.colors.0.len(), LAYERS);
        assert_eq!(appearance.colors.0[0].as_ref(), Some(&BASE));
        assert_eq!(appearance.colors.0[1], None);
    }

    #[test]
    fn a_long_name_is_cut_to_the_name_capacity() {
        let long = preset("a name that is much longer than thirty-two bytes", 0);
        assert_eq!(long.info().name.len(), NAME_LEN);
    }
}
