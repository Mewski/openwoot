use openwoot_protocol::keys::{Action, Keymap, Layer};

use crate::keys::Key;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patch {
    pub key: Key,
    pub action: Action,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DefaultKeymap<const ROWS: usize, const COLUMNS: usize> {
    pub layers: &'static [[[Action; COLUMNS]; ROWS]],
    pub variant_patches: &'static [&'static [Patch]],
}

impl<const ROWS: usize, const COLUMNS: usize> DefaultKeymap<ROWS, COLUMNS> {
    #[must_use]
    pub fn action(&self, layer: Layer, key: Key, variant: u8) -> Action {
        if layer == Layer::Base
            && let Some(patch) = self
                .variant_patches
                .get(usize::from(variant))
                .and_then(|patches| patches.iter().find(|patch| patch.key == key))
        {
            return patch.action;
        }
        self.layers
            .get(usize::from(layer.number()))
            .and_then(|rows| rows.get(key.row()))
            .and_then(|row| row.get(key.column()))
            .copied()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn keymap(&self, layer: Layer, variant: u8) -> Keymap {
        (0..ROWS)
            .flat_map(|row| (0..COLUMNS).map(move |column| (row, column)))
            .filter_map(|(row, column)| {
                Some(Key::new(
                    u8::try_from(row).ok()?,
                    u8::try_from(column).ok()?,
                ))
            })
            .map(|key| self.action(layer, key, variant))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYERS: [[[Action; 2]; 1]; 2] = [
        [[Action::Keyboard(0x04), Action::Keyboard(0x05)]],
        [[Action::None, Action::Keyboard(0x06)]],
    ];
    const VARIANT: [Patch; 1] = [Patch {
        key: Key::new(0, 1),
        action: Action::Keyboard(0x89),
    }];
    const KEYMAP: DefaultKeymap<1, 2> = DefaultKeymap {
        layers: &LAYERS,
        variant_patches: &[&[], &VARIANT],
    };

    #[test]
    fn variants_patch_the_base_layer_only() {
        assert_eq!(
            KEYMAP.keymap(Layer::Base, 0).as_slice(),
            [Action::Keyboard(0x04), Action::Keyboard(0x05)]
        );
        assert_eq!(
            KEYMAP.keymap(Layer::Base, 1).as_slice(),
            [Action::Keyboard(0x04), Action::Keyboard(0x89)]
        );
        assert_eq!(
            KEYMAP.keymap(Layer::One, 1).as_slice(),
            [Action::None, Action::Keyboard(0x06)]
        );
        assert_eq!(
            KEYMAP.keymap(Layer::Two, 0).as_slice(),
            [Action::None, Action::None]
        );
    }
}
