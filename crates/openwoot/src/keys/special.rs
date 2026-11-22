use openwoot_protocol::keys::{Action, Function, Layer};

use super::Key;

pub(crate) const SPECIALS: usize = 15;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    BrightnessUp,
    BrightnessDown,
    Profile(u8),
    ToggleModeProfile,
    Layer(u8),
    ToggleKeyDisable,
    LayerLock,
    NextProfile,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Call {
    pub special: Special,
    pub key: Key,
    pub layer: u8,
    pub pressed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Request {
    pub(super) key: Key,
    pub(super) layer: u8,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Requests([Option<Request>; SPECIALS]);

impl Special {
    const PROFILE_CODES: [u8; 4] = [6, 3, 4, 5];

    #[must_use]
    pub fn from_action(action: Action) -> Option<Self> {
        match action {
            Action::Layer(Layer::Base) => None,
            Action::Layer(layer) => Some(Self::Layer(layer.number())),
            Action::Function(function) => match function {
                Function::BrightnessUp => Some(Self::BrightnessUp),
                Function::BrightnessDown => Some(Self::BrightnessDown),
                Function::Profile(slot) => (usize::from(slot.get()) < Self::PROFILE_CODES.len())
                    .then_some(Self::Profile(slot.get())),
                Function::NextProfile => Some(Self::NextProfile),
                Function::ToggleModeProfile => Some(Self::ToggleModeProfile),
                Function::ToggleKeyDisable => Some(Self::ToggleKeyDisable),
                Function::LayerLock => Some(Self::LayerLock),
            },
            _ => None,
        }
    }

    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::BrightnessUp),
            2 => Some(Self::BrightnessDown),
            3 => Some(Self::Profile(1)),
            4 => Some(Self::Profile(2)),
            5 => Some(Self::Profile(3)),
            6 => Some(Self::Profile(0)),
            8 => Some(Self::ToggleModeProfile),
            9 => Some(Self::Layer(1)),
            10 => Some(Self::ToggleKeyDisable),
            11 => Some(Self::LayerLock),
            12 => Some(Self::Layer(2)),
            13 => Some(Self::Layer(3)),
            14 => Some(Self::NextProfile),
            _ => None,
        }
    }

    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::BrightnessUp => 1,
            Self::BrightnessDown => 2,
            Self::Profile(slot) => Self::PROFILE_CODES
                .get(usize::from(slot))
                .copied()
                .unwrap_or_default(),
            Self::ToggleModeProfile => 8,
            Self::Layer(1) => 9,
            Self::ToggleKeyDisable => 10,
            Self::LayerLock => 11,
            Self::Layer(2) => 12,
            Self::Layer(3) => 13,
            Self::NextProfile => 14,
            Self::Layer(_) => 0,
        }
    }
}

impl Requests {
    pub(super) fn get(&self, code: u8) -> Option<Request> {
        self.0.get(usize::from(code)).copied().flatten()
    }

    pub(super) fn set(&mut self, code: u8, request: Option<Request>) {
        if let Some(slot) = self.0.get_mut(usize::from(code)) {
            *slot = request;
        }
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::profile::Slot;

    use super::*;

    #[test]
    fn every_special_action_keeps_its_c_code() {
        for code in 0..=15 {
            if let Some(special) = Special::from_code(code) {
                assert_eq!(special.code(), code);
            }
        }
        assert_eq!(
            Special::from_action(Action::Function(Function::Profile(Slot::new(0)))),
            Some(Special::Profile(0))
        );
        assert_eq!(
            Special::from_action(Action::Function(Function::Profile(Slot::new(4)))),
            None
        );
        assert_eq!(
            Special::from_action(Action::Layer(Layer::Two)).map(Special::code),
            Some(12)
        );
        assert_eq!(Special::from_action(Action::Keyboard(4)), None);
    }
}
