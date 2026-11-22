use openwoot_protocol::{gamepad::Control, keys::Action};

use super::{Actuation, DEFAULT_MINIMUM, RapidTriggerOverride};
use crate::layout::LAYERS;

#[expect(
    clippy::struct_excessive_bools,
    reason = "the key flags of the C key record are independent states"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Record {
    pub(super) present: bool,
    pub(super) actuation: Actuation,
    pub(super) actuation_point: Option<u16>,
    pub(super) rapid_trigger: RapidTriggerOverride,
    pub(super) minimum: u16,
    pub(super) travel_limit: u16,
    pub(super) actions: [Action; LAYERS],
    pub(super) gamepad: Option<Control>,
    pub(super) advanced: [Option<u8>; LAYERS],
    pub(super) active: [bool; LAYERS],
    pub(super) latched_layer: u8,
    pub(super) disabled: bool,
    pub(super) ignore_until_release: bool,
    pub(super) defer_output: bool,
}

impl Record {
    pub(super) const EMPTY: Self = Self {
        present: false,
        actuation: Actuation::new(),
        actuation_point: None,
        rapid_trigger: RapidTriggerOverride::Profile,
        minimum: DEFAULT_MINIMUM,
        travel_limit: 0,
        actions: [Action::None; LAYERS],
        gamepad: None,
        advanced: [None; LAYERS],
        active: [false; LAYERS],
        latched_layer: 0,
        disabled: false,
        ignore_until_release: false,
        defer_output: false,
    };

    pub(super) fn action(&self, layer: u8) -> Action {
        self.actions
            .get(usize::from(layer))
            .copied()
            .unwrap_or_default()
    }

    pub(super) fn advanced(&self, layer: u8) -> Option<u8> {
        self.advanced.get(usize::from(layer)).copied().flatten()
    }

    pub(super) fn any_active(&self) -> bool {
        self.advanced
            .iter()
            .zip(self.active)
            .any(|(index, active)| index.is_some() && active)
    }

    pub(super) fn holds_events(&self) -> bool {
        self.ignore_until_release || self.any_active()
    }

    pub(super) fn set_active(&mut self, layer: u8, active: bool) {
        if let Some(slot) = self.active.get_mut(usize::from(layer)) {
            *slot = active;
        }
    }
}
