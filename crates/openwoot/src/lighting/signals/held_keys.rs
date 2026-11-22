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

    pub(crate) fn any(&self) -> bool {
        self.0.iter().any(|word| *word != 0)
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
}
