use gpui_kit::SharedString;
use openwoot_protocol::{
    keys::{Action, Function, Keymap, Layer},
    profile::Slot,
};

use crate::usage;

pub(super) fn shortcut(keymaps: &[(Layer, Keymap)], slot: Slot) -> Vec<SharedString> {
    let target = Action::Function(Function::Profile(slot));
    let base = keymaps
        .iter()
        .find(|(layer, _)| *layer == Layer::Base)
        .map(|(_, keymap)| keymap);
    keymaps
        .iter()
        .find_map(|(layer, keymap)| {
            let key = keymap.iter().position(|action| *action == target)?;
            let legend = base
                .and_then(|base| base.get(key))
                .filter(|action| **action != target)
                .map_or_else(|| usage::legend(target), |action| usage::legend(*action));
            let mut chips = Vec::with_capacity(2);
            if *layer != Layer::Base {
                let modifier = base
                    .and_then(|base| base.iter().find(|action| **action == Action::Layer(*layer)))
                    .copied()
                    .unwrap_or(Action::Layer(*layer));
                chips.push(SharedString::from(usage::legend(modifier).into_owned()));
            }
            chips.push(SharedString::from(legend.into_owned()));
            Some(chips)
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const USAGE_ONE: u8 = 0x1E;
    const USAGE_TWO: u8 = 0x1F;

    fn keymap(actions: &[Action]) -> Keymap {
        actions.iter().copied().collect()
    }

    #[test]
    fn a_profile_key_on_the_fn_layer_reads_fn_and_its_key() {
        let keymaps = [
            (
                Layer::Base,
                keymap(&[
                    Action::Keyboard(USAGE_ONE),
                    Action::Keyboard(USAGE_TWO),
                    Action::Layer(Layer::One),
                ]),
            ),
            (
                Layer::One,
                keymap(&[
                    Action::Function(Function::Profile(Slot::new(0))),
                    Action::Function(Function::Profile(Slot::new(1))),
                    Action::None,
                ]),
            ),
        ];
        assert_eq!(shortcut(&keymaps, Slot::new(1)), ["Fn", "2"]);
        assert_eq!(shortcut(&keymaps, Slot::new(2)), Vec::<SharedString>::new());
    }

    #[test]
    fn a_profile_key_on_the_base_layer_is_one_chip() {
        let keymaps = [(
            Layer::Base,
            keymap(&[Action::Function(Function::Profile(Slot::new(3)))]),
        )];
        assert_eq!(shortcut(&keymaps, Slot::new(3)), ["P4"]);
    }
}
