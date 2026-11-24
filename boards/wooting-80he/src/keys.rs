use openwoot::{
    analog::SwitchModel,
    keys::Key,
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
pub const LAYOUT_VARIANTS: usize = 3;
pub const KEYMAP: DefaultKeymap<KEY_ROWS, KEY_COLUMNS> = DefaultKeymap {
    layers: &LAYERS,
    variant_patches: &VARIANT_PATCHES,
};
pub static GEOMETRY: [Geometry; LAYOUT_VARIANTS] = [ANSI_GEOMETRY, ISO_GEOMETRY, JIS_GEOMETRY];

const LAYERS: [[[Action; KEY_COLUMNS]; KEY_ROWS]; 2] = [
    [
        [
            Action::Keyboard(0x29),
            Action::Keyboard(0x3A),
            Action::Keyboard(0x3B),
            Action::Keyboard(0x3C),
            Action::Keyboard(0x3D),
            Action::Keyboard(0x3E),
            Action::Keyboard(0x3F),
            Action::Keyboard(0x40),
            Action::Keyboard(0x41),
            Action::None,
            Action::Keyboard(0x42),
            Action::Keyboard(0x43),
            Action::Keyboard(0x44),
            Action::Keyboard(0x45),
            Action::Function(Function::ToggleModeProfile),
            Action::Keyboard(0x46),
            Action::Keyboard(0x48),
        ],
        [
            Action::Keyboard(0x35),
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
            Action::Keyboard(0x89),
            Action::Keyboard(0x2A),
            Action::Keyboard(0x49),
            Action::Keyboard(0x4B),
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
            Action::None,
            Action::Keyboard(0x31),
            Action::Keyboard(0x4C),
            Action::Keyboard(0x4E),
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
            Action::None,
            Action::Keyboard(0x28),
            Action::None,
            Action::None,
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
            Action::Keyboard(0x87),
            Action::None,
            Action::Keyboard(0xE5),
            Action::Keyboard(0x52),
            Action::None,
        ],
        [
            Action::Keyboard(0xE0),
            Action::Keyboard(0xE3),
            Action::Keyboard(0xE2),
            Action::Keyboard(0x8B),
            Action::Keyboard(0x2C),
            Action::None,
            Action::Keyboard(0x2C),
            Action::None,
            Action::Keyboard(0x2C),
            Action::Keyboard(0x8A),
            Action::Keyboard(0xE6),
            Action::Keyboard(0xE4),
            Action::Layer(Layer::One),
            Action::None,
            Action::Keyboard(0x50),
            Action::Keyboard(0x51),
            Action::Keyboard(0x4F),
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
            Action::Function(Function::BrightnessDown),
            Action::Function(Function::BrightnessUp),
            Action::Consumer(0x00E2),
            Action::Consumer(0x00EA),
            Action::Consumer(0x00E9),
        ],
        [
            Action::None,
            Action::Function(Function::Profile(Slot::new(0))),
            Action::Function(Function::Profile(Slot::new(1))),
            Action::Function(Function::Profile(Slot::new(2))),
            Action::Function(Function::Profile(Slot::new(3))),
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
            Action::Consumer(0x00B6),
            Action::Consumer(0x00B5),
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
            Action::None,
            Action::Consumer(0x00B7),
            Action::Consumer(0x00CD),
        ],
        [
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::Consumer(0x00B6),
            Action::Consumer(0x00CD),
            Action::Consumer(0x00B5),
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
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
            Action::None,
            Action::None,
            Action::None,
            Action::Keyboard(0xE5),
            Action::Keyboard(0x4B),
            Action::None,
        ],
        [
            Action::Keyboard(0xE0),
            Action::Function(Function::ToggleKeyDisable),
            Action::Keyboard(0xE2),
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::None,
            Action::Keyboard(0xE6),
            Action::Keyboard(0xE4),
            Action::Layer(Layer::One),
            Action::None,
            Action::Keyboard(0x4A),
            Action::Keyboard(0x4E),
            Action::Keyboard(0x4D),
        ],
    ],
];

const VARIANT_PATCHES: [&[Patch]; LAYOUT_VARIANTS] = [
    &[],
    &[],
    &[
        Patch {
            layer: Layer::Base,
            key: Key::new(5, 10),
            action: Action::Keyboard(0x88),
        },
        Patch {
            layer: Layer::Base,
            key: Key::new(5, 11),
            action: Action::Keyboard(0xE6),
        },
        Patch {
            layer: Layer::One,
            key: Key::new(5, 10),
            action: Action::None,
        },
        Patch {
            layer: Layer::One,
            key: Key::new(5, 11),
            action: Action::Keyboard(0xE6),
        },
    ],
];

type Geometry = [[Option<geometry::Key>; KEY_COLUMNS]; KEY_ROWS];

const ANSI_GEOMETRY: Geometry = [
    [
        Some(key(0, 0, 100)),
        Some(key(125, 0, 100)),
        Some(key(225, 0, 100)),
        Some(key(325, 0, 100)),
        Some(key(425, 0, 100)),
        Some(key(550, 0, 100)),
        Some(key(650, 0, 100)),
        Some(key(750, 0, 100)),
        Some(key(850, 0, 100)),
        None,
        Some(key(975, 0, 100)),
        Some(key(1075, 0, 100)),
        Some(key(1175, 0, 100)),
        Some(key(1275, 0, 100)),
        Some(key(1400, 0, 100)),
        Some(key(1525, 0, 100)),
        Some(key(1625, 0, 100)),
    ],
    [
        Some(key(0, 125, 100)),
        Some(key(100, 125, 100)),
        Some(key(200, 125, 100)),
        Some(key(300, 125, 100)),
        Some(key(400, 125, 100)),
        Some(key(500, 125, 100)),
        Some(key(600, 125, 100)),
        Some(key(700, 125, 100)),
        Some(key(800, 125, 100)),
        Some(key(900, 125, 100)),
        Some(key(1000, 125, 100)),
        Some(key(1100, 125, 100)),
        Some(key(1200, 125, 100)),
        None,
        Some(key(1300, 125, 200)),
        Some(key(1525, 125, 100)),
        Some(key(1625, 125, 100)),
    ],
    [
        Some(key(0, 225, 150)),
        Some(key(150, 225, 100)),
        Some(key(250, 225, 100)),
        Some(key(350, 225, 100)),
        Some(key(450, 225, 100)),
        Some(key(550, 225, 100)),
        Some(key(650, 225, 100)),
        Some(key(750, 225, 100)),
        Some(key(850, 225, 100)),
        Some(key(950, 225, 100)),
        Some(key(1050, 225, 100)),
        Some(key(1150, 225, 100)),
        Some(key(1250, 225, 100)),
        None,
        Some(key(1350, 225, 150)),
        Some(key(1525, 225, 100)),
        Some(key(1625, 225, 100)),
    ],
    [
        Some(key(0, 325, 175)),
        Some(key(175, 325, 100)),
        Some(key(275, 325, 100)),
        Some(key(375, 325, 100)),
        Some(key(475, 325, 100)),
        Some(key(575, 325, 100)),
        Some(key(675, 325, 100)),
        Some(key(775, 325, 100)),
        Some(key(875, 325, 100)),
        Some(key(975, 325, 100)),
        Some(key(1075, 325, 100)),
        Some(key(1175, 325, 100)),
        None,
        None,
        Some(key(1275, 325, 225)),
        None,
        None,
    ],
    [
        Some(key(0, 425, 225)),
        None,
        Some(key(225, 425, 100)),
        Some(key(325, 425, 100)),
        Some(key(425, 425, 100)),
        Some(key(525, 425, 100)),
        Some(key(625, 425, 100)),
        Some(key(725, 425, 100)),
        Some(key(825, 425, 100)),
        Some(key(925, 425, 100)),
        Some(key(1025, 425, 100)),
        Some(key(1125, 425, 100)),
        None,
        None,
        Some(key(1225, 425, 275)),
        Some(key(1525, 425, 100)),
        None,
    ],
    [
        Some(key(0, 525, 125)),
        Some(key(125, 525, 125)),
        Some(key(250, 525, 125)),
        None,
        None,
        None,
        Some(key(375, 525, 625)),
        None,
        None,
        None,
        Some(key(1000, 525, 125)),
        Some(key(1125, 525, 125)),
        Some(key(1250, 525, 125)),
        None,
        Some(key(1425, 525, 100)),
        Some(key(1525, 525, 100)),
        Some(key(1625, 525, 100)),
    ],
];

const ISO_GEOMETRY: Geometry = [
    [
        Some(key(0, 0, 100)),
        Some(key(125, 0, 100)),
        Some(key(225, 0, 100)),
        Some(key(325, 0, 100)),
        Some(key(425, 0, 100)),
        Some(key(550, 0, 100)),
        Some(key(650, 0, 100)),
        Some(key(750, 0, 100)),
        Some(key(850, 0, 100)),
        None,
        Some(key(975, 0, 100)),
        Some(key(1075, 0, 100)),
        Some(key(1175, 0, 100)),
        Some(key(1275, 0, 100)),
        Some(key(1400, 0, 100)),
        Some(key(1525, 0, 100)),
        Some(key(1625, 0, 100)),
    ],
    [
        Some(key(0, 125, 100)),
        Some(key(100, 125, 100)),
        Some(key(200, 125, 100)),
        Some(key(300, 125, 100)),
        Some(key(400, 125, 100)),
        Some(key(500, 125, 100)),
        Some(key(600, 125, 100)),
        Some(key(700, 125, 100)),
        Some(key(800, 125, 100)),
        Some(key(900, 125, 100)),
        Some(key(1000, 125, 100)),
        Some(key(1100, 125, 100)),
        Some(key(1200, 125, 100)),
        None,
        Some(key(1300, 125, 200)),
        Some(key(1525, 125, 100)),
        Some(key(1625, 125, 100)),
    ],
    [
        Some(key(0, 225, 150)),
        Some(key(150, 225, 100)),
        Some(key(250, 225, 100)),
        Some(key(350, 225, 100)),
        Some(key(450, 225, 100)),
        Some(key(550, 225, 100)),
        Some(key(650, 225, 100)),
        Some(key(750, 225, 100)),
        Some(key(850, 225, 100)),
        Some(key(950, 225, 100)),
        Some(key(1050, 225, 100)),
        Some(key(1150, 225, 100)),
        Some(key(1250, 225, 100)),
        None,
        None,
        Some(key(1525, 225, 100)),
        Some(key(1625, 225, 100)),
    ],
    [
        Some(key(0, 325, 175)),
        Some(key(175, 325, 100)),
        Some(key(275, 325, 100)),
        Some(key(375, 325, 100)),
        Some(key(475, 325, 100)),
        Some(key(575, 325, 100)),
        Some(key(675, 325, 100)),
        Some(key(775, 325, 100)),
        Some(key(875, 325, 100)),
        Some(key(975, 325, 100)),
        Some(key(1075, 325, 100)),
        Some(key(1175, 325, 100)),
        Some(key(1275, 325, 100)),
        None,
        Some(iso_enter(1350, 225)),
        None,
        None,
    ],
    [
        Some(key(0, 425, 125)),
        Some(key(125, 425, 100)),
        Some(key(225, 425, 100)),
        Some(key(325, 425, 100)),
        Some(key(425, 425, 100)),
        Some(key(525, 425, 100)),
        Some(key(625, 425, 100)),
        Some(key(725, 425, 100)),
        Some(key(825, 425, 100)),
        Some(key(925, 425, 100)),
        Some(key(1025, 425, 100)),
        Some(key(1125, 425, 100)),
        None,
        None,
        Some(key(1225, 425, 275)),
        Some(key(1525, 425, 100)),
        None,
    ],
    [
        Some(key(0, 525, 125)),
        Some(key(125, 525, 125)),
        Some(key(250, 525, 125)),
        None,
        None,
        None,
        Some(key(375, 525, 625)),
        None,
        None,
        None,
        Some(key(1000, 525, 125)),
        Some(key(1125, 525, 125)),
        Some(key(1250, 525, 125)),
        None,
        Some(key(1425, 525, 100)),
        Some(key(1525, 525, 100)),
        Some(key(1625, 525, 100)),
    ],
];

const JIS_GEOMETRY: Geometry = [
    [
        Some(key(0, 0, 100)),
        Some(key(125, 0, 100)),
        Some(key(225, 0, 100)),
        Some(key(325, 0, 100)),
        Some(key(425, 0, 100)),
        Some(key(550, 0, 100)),
        Some(key(650, 0, 100)),
        Some(key(750, 0, 100)),
        Some(key(850, 0, 100)),
        None,
        Some(key(975, 0, 100)),
        Some(key(1075, 0, 100)),
        Some(key(1175, 0, 100)),
        Some(key(1275, 0, 100)),
        Some(key(1400, 0, 100)),
        Some(key(1525, 0, 100)),
        Some(key(1625, 0, 100)),
    ],
    [
        Some(key(0, 125, 100)),
        Some(key(100, 125, 100)),
        Some(key(200, 125, 100)),
        Some(key(300, 125, 100)),
        Some(key(400, 125, 100)),
        Some(key(500, 125, 100)),
        Some(key(600, 125, 100)),
        Some(key(700, 125, 100)),
        Some(key(800, 125, 100)),
        Some(key(900, 125, 100)),
        Some(key(1000, 125, 100)),
        Some(key(1100, 125, 100)),
        Some(key(1200, 125, 100)),
        Some(key(1300, 125, 100)),
        Some(key(1400, 125, 100)),
        Some(key(1525, 125, 100)),
        Some(key(1625, 125, 100)),
    ],
    [
        Some(key(0, 225, 150)),
        Some(key(150, 225, 100)),
        Some(key(250, 225, 100)),
        Some(key(350, 225, 100)),
        Some(key(450, 225, 100)),
        Some(key(550, 225, 100)),
        Some(key(650, 225, 100)),
        Some(key(750, 225, 100)),
        Some(key(850, 225, 100)),
        Some(key(950, 225, 100)),
        Some(key(1050, 225, 100)),
        Some(key(1150, 225, 100)),
        Some(key(1250, 225, 100)),
        None,
        None,
        Some(key(1525, 225, 100)),
        Some(key(1625, 225, 100)),
    ],
    [
        Some(key(0, 325, 175)),
        Some(key(175, 325, 100)),
        Some(key(275, 325, 100)),
        Some(key(375, 325, 100)),
        Some(key(475, 325, 100)),
        Some(key(575, 325, 100)),
        Some(key(675, 325, 100)),
        Some(key(775, 325, 100)),
        Some(key(875, 325, 100)),
        Some(key(975, 325, 100)),
        Some(key(1075, 325, 100)),
        Some(key(1175, 325, 100)),
        Some(key(1275, 325, 100)),
        None,
        Some(iso_enter(1350, 225)),
        None,
        None,
    ],
    [
        Some(key(0, 425, 225)),
        None,
        Some(key(225, 425, 100)),
        Some(key(325, 425, 100)),
        Some(key(425, 425, 100)),
        Some(key(525, 425, 100)),
        Some(key(625, 425, 100)),
        Some(key(725, 425, 100)),
        Some(key(825, 425, 100)),
        Some(key(925, 425, 100)),
        Some(key(1025, 425, 100)),
        Some(key(1125, 425, 100)),
        Some(key(1225, 425, 100)),
        None,
        Some(key(1325, 425, 175)),
        Some(key(1525, 425, 100)),
        None,
    ],
    [
        Some(key(0, 525, 125)),
        Some(key(125, 525, 100)),
        Some(key(225, 525, 125)),
        Some(key(350, 525, 100)),
        None,
        None,
        Some(key(450, 525, 450)),
        None,
        None,
        Some(key(900, 525, 125)),
        Some(key(1025, 525, 125)),
        Some(key(1150, 525, 125)),
        Some(key(1275, 525, 100)),
        None,
        Some(key(1425, 525, 100)),
        Some(key(1525, 525, 100)),
        Some(key(1625, 525, 100)),
    ],
];

#[must_use]
pub fn capabilities(edition: Option<u8>) -> Capabilities {
    Capabilities {
        key_rows: u8::try_from(KEY_ROWS).unwrap_or(u8::MAX),
        key_columns: u8::try_from(KEY_COLUMNS).unwrap_or(u8::MAX),
        profiles: PROFILES,
        layouts: Vec::from_iter([Layout::Ansi, Layout::Iso, Layout::Jis]),
        lightbar_leds: u8::try_from(crate::lighting::LIGHTBAR_LEDS.len()).unwrap_or(u8::MAX),
        host_signals: true,
        switch_models: SwitchModel::COUNT,
        edition,
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
        let capabilities = capabilities(None);
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

    #[test]
    fn the_jis_layout_writes_its_two_extra_keys_over_the_base_layer() {
        assert_eq!(
            KEYMAP.action(Layer::Base, Key::new(5, 10), 2),
            Action::Keyboard(0x88)
        );
        assert_eq!(
            KEYMAP.action(Layer::Base, Key::new(5, 10), 0),
            Action::Keyboard(0xE6)
        );
    }

    #[test]
    fn a_modifier_key_stays_that_modifier_while_fn_is_held() {
        for (variant, map) in (0..).zip(SENSOR_MAPS) {
            for (_, key) in map.iter() {
                let held = KEYMAP.action(Layer::One, key, variant);
                if matches!(held, Action::Keyboard(0xE0..=0xE7)) {
                    assert_eq!(held, KEYMAP.action(Layer::Base, key, variant), "{key:?}");
                }
            }
        }
        assert_eq!(KEYMAP.action(Layer::One, Key::new(5, 10), 2), Action::None);
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
    fn the_mode_key_and_the_fn_shortcuts_for_profiles_and_brightness_are_bound() {
        for variant in 0..u8::try_from(LAYOUT_VARIANTS).unwrap() {
            assert_eq!(
                KEYMAP.action(Layer::Base, Key::new(0, 14), variant),
                Action::Function(Function::ToggleModeProfile)
            );
            for (column, slot) in (1..).zip(0..PROFILES) {
                assert_eq!(
                    KEYMAP.action(Layer::One, Key::new(1, column), variant),
                    Action::Function(Function::Profile(Slot::new(slot)))
                );
            }
            assert_eq!(
                KEYMAP.action(Layer::One, Key::new(0, 12), variant),
                Action::Function(Function::BrightnessDown)
            );
            assert_eq!(
                KEYMAP.action(Layer::One, Key::new(0, 13), variant),
                Action::Function(Function::BrightnessUp)
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
            for x in (quarter / 2..1725).step_by(usize::from(quarter)) {
                for y in (quarter / 2..625).step_by(usize::from(quarter)) {
                    let covering = keys_of(layout).filter(|key| key.contains(x, y)).count();
                    assert!(covering <= 1, "({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn every_layout_spans_seventeen_and_a_quarter_by_six_and_a_quarter_units() {
        for layout in &GEOMETRY {
            assert_eq!(keys_of(layout).map(|key| key.x).min(), Some(0));
            assert_eq!(keys_of(layout).map(|key| key.y).min(), Some(0));
            assert_eq!(
                keys_of(layout).map(|key| key.x + key.width).max(),
                Some(1725)
            );
            assert_eq!(
                keys_of(layout).map(|key| key.y + key.height).max(),
                Some(625)
            );
        }
    }

    #[test]
    fn the_function_row_leaves_a_quarter_unit_between_its_groups_and_above_the_digits() {
        for layout in &GEOMETRY {
            let row: Vec<u16, KEY_COLUMNS> = layout[0].iter().flatten().map(|key| key.x).collect();
            assert_eq!(
                row,
                [
                    0, 125, 225, 325, 425, 550, 650, 750, 850, 975, 1075, 1175, 1275, 1400, 1525,
                    1625
                ]
            );
            assert!(layout[1].iter().flatten().all(|key| key.y == 125));
        }
    }

    #[test]
    fn iso_and_jis_have_the_iso_enter_and_jis_a_shorter_space_bar() {
        let [ansi, iso, jis] = &GEOMETRY;
        assert_eq!(ansi[3][14], Some(key(1275, 325, 225)));
        assert!(keys_of(ansi).all(|key| key.shape == Shape::Rectangle));
        for layout in [iso, jis] {
            assert_eq!(layout[3][14], Some(iso_enter(1350, 225)));
            assert_eq!(
                keys_of(layout)
                    .filter(|key| key.shape == Shape::IsoEnter)
                    .count(),
                1
            );
        }
        for (layout, width) in [(ansi, 625), (iso, 625), (jis, 450)] {
            assert_eq!(layout[5][6].map(|key| key.width), Some(width));
        }
    }
}
