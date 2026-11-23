use crate::keys::Key;

const ROW_STRIDE: usize = 32;
const WORDS: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HeldKeys([u32; WORDS]);

impl HeldKeys {
    pub(crate) const fn new() -> Self {
        Self([0; WORDS])
    }

    pub(crate) fn set(&mut self, key: Key, held: bool) {
        let Some(bit) = bit_of(key) else {
            return;
        };
        if let Some(word) = self.0.get_mut(bit / 32) {
            let mask = 1 << (bit % 32);
            if held {
                *word |= mask;
            } else {
                *word &= !mask;
            }
        }
    }

    pub(crate) fn holds(&self, key: Key) -> bool {
        bit_of(key).is_some_and(|bit| {
            self.0
                .get(bit / 32)
                .is_some_and(|word| *word & (1 << (bit % 32)) != 0)
        })
    }

    pub(crate) fn any(&self) -> bool {
        self.0.iter().any(|word| *word != 0)
    }

    pub(crate) const fn clear(&mut self) {
        self.0 = [0; WORDS];
    }
}

fn bit_of(key: Key) -> Option<usize> {
    (key.column() < ROW_STRIDE)
        .then(|| key.row() * ROW_STRIDE + key.column())
        .filter(|bit| *bit < WORDS * 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_counts_until_it_is_released() {
        let mut held = HeldKeys::new();
        held.set(Key::new(5, 16), true);
        held.set(Key::new(0, 0), true);
        held.set(Key::new(0, 0), false);
        assert!(held.any());
        held.set(Key::new(5, 16), false);
        assert!(!held.any());
        held.set(Key::new(9, 0), true);
        assert!(!held.any());
    }

    #[test]
    fn a_key_is_held_from_its_press_to_its_release() {
        let mut held = HeldKeys::new();
        let key = Key::new(2, 3);
        assert!(!held.holds(key));
        held.set(key, true);
        assert!(held.holds(key));
        assert!(!held.holds(Key::new(2, 4)));
        assert!(!held.holds(Key::new(9, 0)));
        held.set(key, false);
        assert!(!held.holds(key));
    }

    #[test]
    fn clearing_lets_go_of_every_key() {
        let mut held = HeldKeys::new();
        held.set(Key::new(0, 0), true);
        held.set(Key::new(5, 16), true);
        held.clear();
        assert!(!held.any());
        assert!(!held.holds(Key::new(5, 16)));
    }
}
