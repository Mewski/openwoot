use openwoot_protocol::{
    advanced::{self, Behaviour},
    keys::{Action, Keymap, Layer},
    lighting::{Color, Indicators},
};

const CAPS_LOCK: u8 = 0x39;
const SCROLL_LOCK: u8 = 0x47;
const NUM_LOCK: u8 = 0x53;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Lock {
    Caps,
    Scroll,
    Num,
    Layer,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct LockKeys([Option<usize>; 4]);

impl Lock {
    const ALL: [Self; 4] = [Self::Caps, Self::Scroll, Self::Num, Self::Layer];

    pub(super) const fn color(self, indicators: &Indicators) -> Color {
        match self {
            Self::Caps => indicators.caps_lock,
            Self::Scroll => indicators.scroll_lock,
            Self::Num => indicators.num_lock,
            Self::Layer => indicators.layer_lock,
        }
    }

    pub(super) const fn set(self, indicators: &mut Indicators, color: Color) {
        match self {
            Self::Caps => indicators.caps_lock = color,
            Self::Scroll => indicators.scroll_lock = color,
            Self::Num => indicators.num_lock = color,
            Self::Layer => indicators.layer_lock = color,
        }
    }

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Caps => "Caps Lock indicator",
            Self::Scroll => "Scroll Lock indicator",
            Self::Num => "Num Lock indicator",
            Self::Layer => "Layer lock indicator",
        }
    }

    const fn of(action: Action) -> Option<Self> {
        match action {
            Action::Keyboard(CAPS_LOCK) => Some(Self::Caps),
            Action::Keyboard(SCROLL_LOCK) => Some(Self::Scroll),
            Action::Keyboard(NUM_LOCK) => Some(Self::Num),
            Action::Layer(Layer::One) => Some(Self::Layer),
            _ => None,
        }
    }

    const fn slot(self) -> usize {
        match self {
            Self::Caps => 0,
            Self::Scroll => 1,
            Self::Num => 2,
            Self::Layer => 3,
        }
    }
}

impl LockKeys {
    pub(super) fn find(
        keymaps: &[Keymap],
        advanced: &[advanced::Key],
        present: impl Fn(usize) -> bool,
    ) -> Self {
        let mapped = keymaps.iter().flat_map(|keymap| {
            keymap
                .iter()
                .copied()
                .enumerate()
                .filter(|(key, _)| present(*key))
                .map(|(key, action)| (action, key))
        });
        let held = advanced.iter().flat_map(|entry| {
            actions(&entry.behaviour)
                .into_iter()
                .map(move |action| (action, usize::from(entry.key.get())))
        });
        let mut keys = [None; 4];
        for (action, key) in mapped.chain(held) {
            if let Some(slot) = Lock::of(action).and_then(|lock| keys.get_mut(lock.slot()))
                && slot.is_none()
            {
                *slot = Some(key);
            }
        }
        Self(keys)
    }

    pub(super) fn lock_on(self, key: usize) -> Option<Lock> {
        Lock::ALL
            .into_iter()
            .rev()
            .find(|lock| self.0.get(lock.slot()).copied().flatten() == Some(key))
    }
}

fn actions(behaviour: &Behaviour) -> Vec<Action> {
    match behaviour {
        Behaviour::DynamicKeystroke(keystroke) => keystroke
            .strokes
            .iter()
            .map(|stroke| stroke.action)
            .collect(),
        Behaviour::ModTap(mod_tap) => vec![mod_tap.hold, mod_tap.tap],
        Behaviour::Toggle(action) => vec![*action],
        Behaviour::DepthPriority(_) | Behaviour::Socd(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        advanced::{Interrupt, ModTap},
        keys::Index,
    };

    use super::*;

    fn keymap(actions: &[(usize, Action)]) -> Keymap {
        let mut keymap = Keymap::new();
        for _ in 0..8 {
            keymap.push(Action::None).ok();
        }
        for (key, action) in actions {
            if let Some(slot) = keymap.get_mut(*key) {
                *slot = *action;
            }
        }
        keymap
    }

    #[test]
    fn each_indicator_sits_on_the_first_key_with_its_action() {
        let base = keymap(&[
            (2, Action::Keyboard(CAPS_LOCK)),
            (5, Action::Layer(Layer::One)),
            (6, Action::Keyboard(CAPS_LOCK)),
        ]);
        let one = keymap(&[(1, Action::Keyboard(NUM_LOCK))]);
        let locks = LockKeys::find(&[base, one], &[], |_| true);
        assert_eq!(locks.lock_on(2), Some(Lock::Caps));
        assert_eq!(locks.lock_on(6), None);
        assert_eq!(locks.lock_on(5), Some(Lock::Layer));
        assert_eq!(locks.lock_on(1), Some(Lock::Num));
        assert_eq!(locks.lock_on(0), None);
    }

    #[test]
    fn keys_without_a_switch_and_advanced_keys_follow_the_firmware_order() {
        let base = keymap(&[(3, Action::Keyboard(SCROLL_LOCK))]);
        let held = advanced::Key {
            key: Index::new(4),
            layer: Layer::Base,
            behaviour: Behaviour::ModTap(ModTap {
                tap: Action::Keyboard(CAPS_LOCK),
                hold: Action::Layer(Layer::One),
                hold_time_ms: 200,
                interrupt: Interrupt::Auto,
            }),
        };
        let locks = LockKeys::find(&[base], &[held], |key| key != 3);
        assert_eq!(locks.lock_on(3), None);
        assert_eq!(locks.lock_on(4), Some(Lock::Layer));
        let caps_too = LockKeys::find(&[keymap(&[])], &[held], |_| true);
        assert_eq!(caps_too.0, [Some(4), None, None, Some(4)]);
    }
}
