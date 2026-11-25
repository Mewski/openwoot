use openwoot::{
    analog::SwitchModel,
    layout::{DefaultKeymap, Patch},
    scan::Map,
};
use openwoot_protocol::{
    configuration::Capabilities,
    device::Layout,
    geometry::{self, Shape, UNIT},
    heapless::Vec,
    keys::{Action, Function, Layer},
    profile::Slot,
};

use crate::{
    KEY_COLUMNS, KEY_ROWS,
    scan::{ADDRESSES, INPUTS, SENSOR_MAPS},
};

pub const PROFILES: u8 = 4;
pub const LINKED_PROFILES: u8 = 10;
pub const LAYOUT_VARIANTS: usize = 2;
pub const KEYMAP: DefaultKeymap<KEY_ROWS, KEY_COLUMNS> = DefaultKeymap {
    layers: &LAYERS,
    variant_patches: &VARIANT_PATCHES,
};
pub static GEOMETRY: [Geometry; LAYOUT_VARIANTS] = [ANSI_GEOMETRY, ISO_GEOMETRY];

const LAYERS: [[[Action; KEY_COLUMNS]; KEY_ROWS]; 3] = [
    [
        [
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
        ],
        [
            Action::Keyboard(0x29),
            Action::Keyboard(0x1E),
            Action::Keyboard(0x1F),
            Action::Keyboard(0x20),
            Action::Keyboard(0x21),
            Action::Keyboard(0x22),
            Action::Keyboard(0x23),
            Action::Keyboard(0x24),
            Action::Keyboard(0x25),
            Action::Keyboard(0x26),
            Action::Keyboard(0x27),
            Action::Keyboard(0x2D),
            Action::Keyboard(0x2E),
            Action::Keyboard(0x2A),
        ],
        [
            Action::Keyboard(0x2B),
            Action::Keyboard(0x14),
            Action::Keyboard(0x1A),
            Action::Keyboard(0x08),
            Action::Keyboard(0x15),
            Action::Keyboard(0x17),
            Action::Keyboard(0x1C),
            Action::Keyboard(0x18),
            Action::Keyboard(0x0C),
            Action::Keyboard(0x12),
            Action::Keyboard(0x13),
            Action::Keyboard(0x2F),
            Action::Keyboard(0x30),
            Action::Keyboard(0x31),
        ],
        [
            Action::Keyboard(0x39),
            Action::Keyboard(0x04),
            Action::Keyboard(0x16),
            Action::Keyboard(0x07),
            Action::Keyboard(0x09),
            Action::Keyboard(0x0A),
            Action::Keyboard(0x0B),
            Action::Keyboard(0x0D),
            Action::Keyboard(0x0E),
            Action::Keyboard(0x0F),
            Action::Keyboard(0x33),
            Action::Keyboard(0x34),
            Action::Keyboard(0x31),
            Action::Keyboard(0x28),
        ],
        [
            Action::Keyboard(0xE1),
            Action::Keyboard(0x64),
            Action::Keyboard(0x1D),
            Action::Keyboard(0x1B),
            Action::Keyboard(0x06),
            Action::Keyboard(0x19),
            Action::Keyboard(0x05),
            Action::Keyboard(0x11),
            Action::Keyboard(0x10),
            Action::Keyboard(0x36),
            Action::Keyboard(0x37),
            Action::Keyboard(0x38),
            Action::None,
            Action::Keyboard(0xE5),
        ],
        [
            Action::Keyboard(0xE0),
            Action::Keyboard(0xE3),
            Action::Keyboard(0xE2),
            Action::None,
            Action::Keyboard(0x2C),
            Action::None,
            Action::Keyboard(0x2C),
            Action::None,
            Action::Keyboard(0x2C),
            Action::None,
            Action::Keyboard(0xE6),
            Action::Keyboard(0x65),
            Action::Keyboard(0xE4),
            Action::Layer(Layer::One),
        ],
    ],
    [
        [
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
        ],
        [
            Action::Keyboard(0x35),
            Action::Keyboard(0x3A),
            Action::Keyboard(0x3B),
            Action::Keyboard(0x3C),
            Action::Keyboard(0x3D),
            Action::Keyboard(0x3E),
            Action::Keyboard(0x3F),
            Action::Keyboard(0x40),
            Action::Keyboard(0x41),
            Action::Keyboard(0x42),
            Action::Keyboard(0x43),
            Action::Keyboard(0x44),
            Action::Keyboard(0x45),
            Action::Keyboard(0x4C),
        ],
        [
            Action::Keyboard(0x46),
            Action::None,
            Action::Keyboard(0x4B),
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::Function(Function::Profile(Slot::new(1))),
            Action::Function(Function::Profile(Slot::new(2))),
            Action::Function(Function::Profile(Slot::new(3))),
            Action::None,
        ],
        [
            Action::Layer(Layer::Two),
            Action::Keyboard(0x4A),
            Action::Keyboard(0x4E),
            Action::Keyboard(0x4D),
            Action::None,
            Action::None,
            Action::None,
            Action::Consumer(0x00B6),
            Action::Consumer(0x00CD),
            Action::Consumer(0x00B5),
            Action::Function(Function::BrightnessDown),
            Action::Function(Function::BrightnessUp),
            Action::None,
            Action::Function(Function::ToggleModeProfile),
        ],
        [
            Action::Keyboard(0xE1),
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::Consumer(0x00E2),
            Action::Consumer(0x00EA),
            Action::Consumer(0x00E9),
            Action::Keyboard(0x52),
            Action::None,
            Action::Keyboard(0xE5),
        ],
        [
            Action::Keyboard(0xE0),
            Action::Keyboard(0xE3),
            Action::Keyboard(0xE2),
            Action::None,
            Action::Keyboard(0x2C),
            Action::None,
            Action::Keyboard(0x2C),
            Action::None,
            Action::Keyboard(0x2C),
            Action::None,
            Action::Keyboard(0x50),
            Action::Keyboard(0x51),
            Action::Keyboard(0x4F),
            Action::Layer(Layer::One),
        ],
    ],
    [
        [
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
        ],
        [
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
        ],
        [
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::Keyboard(0x08),
            Action::Keyboard(0x13),
            Action::Keyboard(0x0C),
            Action::Keyboard(0x06),
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
        ],
        [
            Action::Layer(Layer::Two),
            Action::None,
            Action::None,
            Action::None,
            Action::Keyboard(0x0A),
            Action::Keyboard(0x04),
            Action::Keyboard(0x10),
            Action::Keyboard(0x08),
            Action::Keyboard(0x15),
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
        ],
        [
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::Keyboard(0x10),
            Action::Keyboard(0x12),
            Action::Keyboard(0x19),
            Action::Keyboard(0x08),
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
        ],
        [
            Action::Keyboard(0x50),
            Action::Function(Function::ToggleKeyDisable),
            Action::Keyboard(0x4F),
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::Layer(Layer::One),
        ],
    ],
];

const VARIANT_PATCHES: [&[Patch]; LAYOUT_VARIANTS] = [&[], &[]];

type Geometry = [[Option<geometry::Key>; KEY_COLUMNS]; KEY_ROWS];

const ANSI_GEOMETRY: Geometry = [
    [None; KEY_COLUMNS],
    [
        Some(key(0, 0, 100)),
        Some(key(100, 0, 100)),
        Some(key(200, 0, 100)),
        Some(key(300, 0, 100)),
        Some(key(400, 0, 100)),
        Some(key(500, 0, 100)),
        Some(key(600, 0, 100)),
        Some(key(700, 0, 100)),
        Some(key(800, 0, 100)),
        Some(key(900, 0, 100)),
        Some(key(1000, 0, 100)),
        Some(key(1100, 0, 100)),
        Some(key(1200, 0, 100)),
        Some(key(1300, 0, 200)),
    ],
    [
        Some(key(0, 100, 150)),
        Some(key(150, 100, 100)),
        Some(key(250, 100, 100)),
        Some(key(350, 100, 100)),
        Some(key(450, 100, 100)),
        Some(key(550, 100, 100)),
        Some(key(650, 100, 100)),
        Some(key(750, 100, 100)),
        Some(key(850, 100, 100)),
        Some(key(950, 100, 100)),
        Some(key(1050, 100, 100)),
        Some(key(1150, 100, 100)),
        Some(key(1250, 100, 100)),
        Some(key(1350, 100, 150)),
    ],
    [
        Some(key(0, 200, 175)),
        Some(key(175, 200, 100)),
        Some(key(275, 200, 100)),
        Some(key(375, 200, 100)),
        Some(key(475, 200, 100)),
        Some(key(575, 200, 100)),
        Some(key(675, 200, 100)),
        Some(key(775, 200, 100)),
        Some(key(875, 200, 100)),
        Some(key(975, 200, 100)),
        Some(key(1075, 200, 100)),
        Some(key(1175, 200, 100)),
        None,
        Some(key(1275, 200, 225)),
    ],
    [
        Some(key(0, 300, 225)),
        None,
        Some(key(225, 300, 100)),
        Some(key(325, 300, 100)),
        Some(key(425, 300, 100)),
        Some(key(525, 300, 100)),
        Some(key(625, 300, 100)),
        Some(key(725, 300, 100)),
        Some(key(825, 300, 100)),
        Some(key(925, 300, 100)),
        Some(key(1025, 300, 100)),
        Some(key(1125, 300, 100)),
        None,
        Some(key(1225, 300, 275)),
    ],
    [
        Some(key(0, 400, 125)),
        Some(key(125, 400, 125)),
        Some(key(250, 400, 125)),
        None,
        None,
        None,
        Some(key(375, 400, 625)),
        None,
        None,
        None,
        Some(key(1000, 400, 125)),
        Some(key(1125, 400, 125)),
        Some(key(1250, 400, 125)),
        Some(key(1375, 400, 125)),
    ],
];

const ISO_GEOMETRY: Geometry = [
    [None; KEY_COLUMNS],
    [
        Some(key(0, 0, 100)),
        Some(key(100, 0, 100)),
        Some(key(200, 0, 100)),
        Some(key(300, 0, 100)),
        Some(key(400, 0, 100)),
        Some(key(500, 0, 100)),
        Some(key(600, 0, 100)),
        Some(key(700, 0, 100)),
        Some(key(800, 0, 100)),
        Some(key(900, 0, 100)),
        Some(key(1000, 0, 100)),
        Some(key(1100, 0, 100)),
        Some(key(1200, 0, 100)),
        Some(key(1300, 0, 200)),
    ],
    [
        Some(key(0, 100, 150)),
        Some(key(150, 100, 100)),
        Some(key(250, 100, 100)),
        Some(key(350, 100, 100)),
        Some(key(450, 100, 100)),
        Some(key(550, 100, 100)),
        Some(key(650, 100, 100)),
        Some(key(750, 100, 100)),
        Some(key(850, 100, 100)),
        Some(key(950, 100, 100)),
        Some(key(1050, 100, 100)),
        Some(key(1150, 100, 100)),
        Some(key(1250, 100, 100)),
        None,
    ],
    [
        Some(key(0, 200, 175)),
        Some(key(175, 200, 100)),
        Some(key(275, 200, 100)),
        Some(key(375, 200, 100)),
        Some(key(475, 200, 100)),
        Some(key(575, 200, 100)),
        Some(key(675, 200, 100)),
        Some(key(775, 200, 100)),
        Some(key(875, 200, 100)),
        Some(key(975, 200, 100)),
        Some(key(1075, 200, 100)),
        Some(key(1175, 200, 100)),
        Some(key(1275, 200, 100)),
        Some(iso_enter(1350, 100)),
    ],
    [
        Some(key(0, 300, 125)),
        Some(key(125, 300, 100)),
        Some(key(225, 300, 100)),
        Some(key(325, 300, 100)),
        Some(key(425, 300, 100)),
        Some(key(525, 300, 100)),
        Some(key(625, 300, 100)),
        Some(key(725, 300, 100)),
        Some(key(825, 300, 100)),
        Some(key(925, 300, 100)),
        Some(key(1025, 300, 100)),
        Some(key(1125, 300, 100)),
        None,
        Some(key(1225, 300, 275)),
    ],
    [
        Some(key(0, 400, 125)),
        Some(key(125, 400, 125)),
        Some(key(250, 400, 125)),
        None,
        None,
        None,
        Some(key(375, 400, 625)),
        None,
        None,
        None,
        Some(key(1000, 400, 125)),
        Some(key(1125, 400, 125)),
        Some(key(1250, 400, 125)),
        Some(key(1375, 400, 125)),
    ],
];

#[must_use]
pub fn capabilities() -> Capabilities {
    Capabilities {
        key_rows: u8::try_from(KEY_ROWS).unwrap_or(u8::MAX),
        key_columns: u8::try_from(KEY_COLUMNS).unwrap_or(u8::MAX),
        profiles: PROFILES,
        linked_profiles: LINKED_PROFILES,
        layouts: Vec::from_iter([Layout::Ansi, Layout::Iso]),
        lightbar_leds: 0,
        host_signals: false,
        switch_models: SwitchModel::COUNT,
        edition: None,
    }
}

#[must_use]
pub fn sensor_map(variant: u8) -> Map<INPUTS, ADDRESSES> {
    SENSOR_MAPS
        .get(usize::from(variant))
        .or(SENSOR_MAPS.first())
        .copied()
        .unwrap_or_else(|| Map::from_codes([[0xFF; 16]; INPUTS]))
}

const fn key(x: u16, y: u16, width: u16) -> geometry::Key {
    geometry::Key {
        x,
        y,
        width,
        height: UNIT,
        shape: Shape::Rectangle,
    }
}

const fn iso_enter(x: u16, y: u16) -> geometry::Key {
    geometry::Key {
        x,
        y,
        width: UNIT + UNIT / 2,
        height: 2 * UNIT,
        shape: Shape::IsoEnter,
    }
}

#[cfg(test)]
mod tests {
    use openwoot::keys::Key;
    use openwoot_protocol::{
        configuration::Validate,
        keys::{Keymap, Layer},
    };

    use super::*;

    #[test]
    fn every_factory_action_is_valid_on_the_keyboard() {
        let capabilities = capabilities();
        for variant in 0..u8::try_from(LAYOUT_VARIANTS).unwrap() {
            for layer in Layer::ALL {
                let keymap: Keymap = KEYMAP.actions(layer, variant).collect();
                assert_eq!(keymap.len(), KEY_ROWS * KEY_COLUMNS);
                for action in keymap {
                    assert_eq!(action.validate(&capabilities), Ok(()), "{action:?}");
                }
            }
        }
    }

    #[test]
    fn the_base_layer_types_every_letter_once_and_has_an_fn_key() {
        for variant in 0..u8::try_from(LAYOUT_VARIANTS).unwrap() {
            let keymap: Keymap = KEYMAP.actions(Layer::Base, variant).collect();
            for letter in 0x04..=0x1D {
                let count = keymap
                    .iter()
                    .filter(|action| **action == Action::Keyboard(letter))
                    .count();
                assert_eq!(count, 1, "usage {letter:#04x}");
            }
            assert!(keymap.contains(&Action::Layer(Layer::One)));
        }
    }

    #[test]
    fn every_factory_key_lies_on_a_sensor_of_some_layout() {
        for (variant, map) in (0..).zip(SENSOR_MAPS) {
            for (_, key) in map.iter() {
                assert!(key.index(KEY_COLUMNS) < KEY_ROWS * KEY_COLUMNS);
            }
            assert_eq!(sensor_map(variant), map);
        }
        assert_eq!(sensor_map(u8::MAX), SENSOR_MAPS[0]);
        assert_eq!(
            KEYMAP.action(Layer::Base, Key::new(5, 10), 0),
            KEYMAP.action(Layer::Base, Key::new(5, 10), 1)
        );
    }

    fn usage(character: u8) -> u8 {
        match character {
            b'a'..=b'z' => character - b'a' + 0x04,
            b'1'..=b'9' => character - b'1' + 0x1E,
            _ => 0x27,
        }
    }

    #[test]
    fn the_base_layer_has_the_digit_and_letter_rows_in_typing_order() {
        for variant in 0..u8::try_from(LAYOUT_VARIANTS).unwrap() {
            for (row, first_column, characters) in [
                (1, 1, "1234567890"),
                (2, 1, "qwertyuiop"),
                (3, 1, "asdfghjkl"),
                (4, 2, "zxcvbnm"),
            ] {
                for (column, character) in (first_column..).zip(characters.bytes()) {
                    assert_eq!(
                        KEYMAP.action(Layer::Base, Key::new(row, column), variant),
                        Action::Keyboard(usage(character)),
                        "{}",
                        char::from(character)
                    );
                }
            }
        }
    }

    #[test]
    fn every_sensor_key_has_a_base_action_of_its_own() {
        for (variant, map) in (0..).zip(SENSOR_MAPS) {
            for (_, key) in map.iter() {
                let action = KEYMAP.action(Layer::Base, key, variant);
                assert_ne!(action, Action::None, "{key:?}");
                let keys = map
                    .iter()
                    .filter(|(_, other)| KEYMAP.action(Layer::Base, *other, variant) == action)
                    .count();
                assert_eq!(keys, 1, "{action:?}");
            }
        }
    }

    #[test]
    fn a_layer_key_selects_the_same_layer_on_the_layer_it_opens() {
        for variant in 0..u8::try_from(LAYOUT_VARIANTS).unwrap() {
            for layer in Layer::ALL {
                for (_, key) in sensor_map(variant).iter() {
                    if let Action::Layer(opened) = KEYMAP.action(layer, key, variant) {
                        assert_eq!(
                            KEYMAP.action(opened, key, variant),
                            Action::Layer(opened),
                            "{key:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_second_fn_layer_has_the_left_arrow_left_of_the_right_arrow() {
        for variant in 0..u8::try_from(LAYOUT_VARIANTS).unwrap() {
            assert_eq!(
                KEYMAP.action(Layer::Two, Key::new(5, 0), variant),
                Action::Keyboard(0x50)
            );
            assert_eq!(
                KEYMAP.action(Layer::Two, Key::new(5, 2), variant),
                Action::Keyboard(0x4F)
            );
        }
    }

    fn keys_of(layout: &Geometry) -> impl Iterator<Item = &geometry::Key> {
        layout.as_flattened().iter().flatten()
    }

    #[test]
    fn exactly_the_keys_with_a_sensor_have_geometry_in_every_layout() {
        for (layout, map) in GEOMETRY.iter().zip(SENSOR_MAPS) {
            for (row, keys) in (0..).zip(layout) {
                for (column, geometry) in (0..).zip(keys) {
                    let key = Key::new(row, column);
                    assert_eq!(geometry.is_some(), map.position(key).is_some(), "{key:?}");
                }
            }
        }
    }

    #[test]
    fn no_two_keys_overlap_and_every_edge_lies_on_a_quarter_unit() {
        let quarter = UNIT / 4;
        for layout in &GEOMETRY {
            for key in keys_of(layout) {
                for edge in [key.x, key.y, key.width, key.height] {
                    assert_eq!(edge % quarter, 0, "{key:?}");
                }
            }
            for x in (quarter / 2..1500).step_by(usize::from(quarter)) {
                for y in (quarter / 2..500).step_by(usize::from(quarter)) {
                    let covering = keys_of(layout).filter(|key| key.contains(x, y)).count();
                    assert!(covering <= 1, "({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn every_layout_spans_fifteen_by_five_units() {
        for layout in &GEOMETRY {
            assert_eq!(keys_of(layout).map(|key| key.x).min(), Some(0));
            assert_eq!(keys_of(layout).map(|key| key.y).min(), Some(0));
            assert_eq!(
                keys_of(layout).map(|key| key.x + key.width).max(),
                Some(1500)
            );
            assert_eq!(
                keys_of(layout).map(|key| key.y + key.height).max(),
                Some(500)
            );
        }
    }

    #[test]
    fn iso_has_the_iso_enter_and_both_layouts_the_long_space_bar() {
        let [ansi, iso] = &GEOMETRY;
        assert_eq!(ansi[3][13], Some(key(1275, 200, 225)));
        assert!(keys_of(ansi).all(|key| key.shape == Shape::Rectangle));
        assert_eq!(iso[3][13], Some(iso_enter(1350, 100)));
        assert_eq!(
            keys_of(iso)
                .filter(|key| key.shape == Shape::IsoEnter)
                .count(),
            1
        );
        for layout in [ansi, iso] {
            assert_eq!(layout[5][6], Some(key(375, 400, 625)));
        }
    }
}
