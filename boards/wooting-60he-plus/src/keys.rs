use openwoot::{
    analog::SwitchType,
    layout::{DefaultKeymap, Patch},
    scan::Map,
};
use openwoot_protocol::{
    configuration::Capabilities,
    device::Layout,
    heapless::Vec,
    keys::{Action, Function, Layer},
    profile::Slot,
};

use crate::{
    KEY_COLUMNS, KEY_ROWS,
    scan::{ADDRESSES, INPUTS, SENSOR_MAPS},
};

pub const PROFILES: u8 = 4;
pub const LAYOUT_VARIANTS: usize = 2;
pub const KEYMAP: DefaultKeymap<KEY_ROWS, KEY_COLUMNS> = DefaultKeymap {
    layers: &LAYERS,
    variant_patches: &VARIANT_PATCHES,
};

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
            Action::Keyboard(0x4F),
            Action::Function(Function::ToggleKeyDisable),
            Action::Keyboard(0x50),
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

#[must_use]
pub fn capabilities() -> Capabilities {
    Capabilities {
        key_rows: u8::try_from(KEY_ROWS).unwrap_or(u8::MAX),
        key_columns: u8::try_from(KEY_COLUMNS).unwrap_or(u8::MAX),
        profiles: PROFILES,
        layouts: Vec::from_iter([Layout::Ansi, Layout::Iso]),
        lightbar_leds: 0,
        host_signals: false,
        switch_models: SwitchType::COUNT,
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
}
