use openwoot_protocol::{
    keys::{Keymap, LAYERS},
    profile::Slot,
};

pub(super) const FIRST: Slot = Slot::new(0);
pub(super) const RELINK_TITLE: &str = "Re-link profile back to the first profile";
pub(super) const RELINK_TEXT: &str = "All custom bindings from this profile will be removed and reverted back to the bindings of your first profile";
pub(super) const RELINK_ACTION: &str = "Re-link profile";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LinkState {
    First,
    Linked,
    Unlinked,
}

impl LinkState {
    pub(super) fn of(profile: Slot, linked: bool) -> Self {
        match (profile == FIRST, linked) {
            (true, _) => Self::First,
            (false, true) => Self::Linked,
            (false, false) => Self::Unlinked,
        }
    }

    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::First => "Remap keys",
            Self::Linked => "Remap keys (linked)",
            Self::Unlinked => "Remap keys (un-linked)",
        }
    }

    pub(super) const fn description(self) -> &'static str {
        match self {
            Self::First => "Changes made here will affect this profile and others that are linked",
            Self::Linked => "This layout is linked to your first onboard profile.",
            Self::Unlinked => "Changes made here only affect this profile",
        }
    }

    pub(super) const fn switch(self) -> Option<bool> {
        match self {
            Self::First => None,
            Self::Linked => Some(true),
            Self::Unlinked => Some(false),
        }
    }

    pub(super) const fn editable(self) -> bool {
        !matches!(self, Self::Linked)
    }
}

pub(super) fn customised(own: &[Option<Keymap>; LAYERS], first: &[Option<Keymap>; LAYERS]) -> bool {
    own.iter()
        .zip(first)
        .any(|(own, first)| own.is_none() || first.is_none() || own != first)
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::keys::Action;

    use super::*;

    fn keymaps(first_key: Action) -> [Option<Keymap>; LAYERS] {
        [
            Some([first_key, Action::Keyboard(0x05)].into_iter().collect()),
            Some([Action::None, Action::None].into_iter().collect()),
            Some([Action::None, Action::None].into_iter().collect()),
            Some([Action::None, Action::None].into_iter().collect()),
        ]
    }

    #[test]
    fn the_first_profile_has_no_switch_and_stays_editable() {
        for linked in [false, true] {
            let state = LinkState::of(FIRST, linked);
            assert_eq!(state, LinkState::First);
            assert_eq!(state.switch(), None);
            assert!(state.editable());
            assert_eq!(state.title(), "Remap keys");
        }
    }

    #[test]
    fn a_linked_profile_is_read_only_and_shows_the_switch_on() {
        for profile in [Slot::new(1), Slot::linked(0)] {
            let state = LinkState::of(profile, true);
            assert_eq!(state, LinkState::Linked);
            assert_eq!(state.switch(), Some(true));
            assert!(!state.editable());
            assert_eq!(state.title(), "Remap keys (linked)");
        }
    }

    #[test]
    fn an_unlinked_profile_is_editable_and_shows_the_switch_off() {
        for profile in [Slot::new(3), Slot::linked(9)] {
            let state = LinkState::of(profile, false);
            assert_eq!(state, LinkState::Unlinked);
            assert_eq!(state.switch(), Some(false));
            assert!(state.editable());
            assert_eq!(state.title(), "Remap keys (un-linked)");
        }
    }

    #[test]
    fn relinking_asks_only_when_the_profile_has_custom_bindings() {
        let first = keymaps(Action::Keyboard(0x04));
        assert!(!customised(&first, &first));
        assert!(customised(&keymaps(Action::Keyboard(0x06)), &first));
    }

    #[test]
    fn relinking_asks_when_a_layer_is_not_loaded() {
        let first = keymaps(Action::Keyboard(0x04));
        let mut own = first.clone();
        if let Some(layer) = own.get_mut(2) {
            *layer = None;
        }
        assert!(customised(&own, &first));
        assert!(customised(&first, &own));
    }
}
