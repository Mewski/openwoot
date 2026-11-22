use crate::keys::Key;

pub const PENDING_SLOTS: usize = 5;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Slot {
    in_use: bool,
    key: Option<Key>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pending {
    slots: [Slot; PENDING_SLOTS],
    top: Option<u8>,
}

impl Pending {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [Slot {
                in_use: false,
                key: None,
            }; PENDING_SLOTS],
            top: None,
        }
    }

    #[must_use]
    pub const fn top(&self) -> Option<u8> {
        self.top
    }

    #[must_use]
    pub fn key(&self, slot: u8) -> Option<Key> {
        self.slots
            .get(usize::from(slot))
            .filter(|slot| slot.in_use)
            .and_then(|slot| slot.key)
    }

    pub fn take(&mut self, key: Key) -> Option<u8> {
        let (index, slot) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| !slot.in_use)?;
        *slot = Slot {
            in_use: true,
            key: Some(key),
        };
        let index = u8::try_from(index).ok()?;
        if self.top.is_none_or(|top| index > top) {
            self.top = Some(index);
        }
        Some(index)
    }

    pub fn release(&mut self, slot: u8) {
        let Some(top) = self.top else {
            return;
        };
        if slot > top {
            return;
        }
        if let Some(entry) = self.slots.get_mut(usize::from(slot)) {
            entry.in_use = false;
        }
        if slot == top {
            self.top = (0..slot).rev().find(|&below| self.key(below).is_some());
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releasing_the_top_slot_lowers_the_top_to_the_next_slot_in_use() {
        let mut pending = Pending::new();
        assert_eq!(pending.take(Key::new(0, 0)), Some(0));
        assert_eq!(pending.take(Key::new(0, 1)), Some(1));
        assert_eq!(pending.take(Key::new(0, 2)), Some(2));
        pending.release(1);
        assert_eq!(pending.top(), Some(2));
        pending.release(2);
        assert_eq!(pending.top(), Some(0));
        assert_eq!(pending.take(Key::new(1, 0)), Some(1));
        pending.release(0);
        pending.release(1);
        assert_eq!(pending.top(), None);
        for column in 0..5 {
            assert!(pending.take(Key::new(2, column)).is_some());
        }
        assert_eq!(pending.take(Key::new(3, 0)), None);
    }
}
