use openwoot_protocol::profile::MAX_LINKED;

use super::record::{RecordKey, Scope};

const PROFILE_SLOTS: u8 = 16;
const ITEMS: u8 = 64;
const GLOBAL_ROW: u8 = PROFILE_SLOTS + MAX_LINKED;
const ROWS: usize = GLOBAL_ROW as usize + 1;

#[derive(Debug)]
pub(super) struct StoredKeys {
    rows: [u64; ROWS],
    complete: bool,
}

impl StoredKeys {
    pub(super) const fn unknown() -> Self {
        Self {
            rows: [0; ROWS],
            complete: false,
        }
    }

    pub(super) const fn forget(&mut self) {
        self.rows = [0; ROWS];
        self.complete = false;
    }

    pub(super) const fn start(&mut self) {
        self.rows = [0; ROWS];
        self.complete = true;
    }

    pub(super) fn insert(&mut self, key: RecordKey) {
        match place(key).and_then(|(row, bit)| Some((self.rows.get_mut(row)?, bit))) {
            Some((row, bit)) => *row |= bit,
            None => self.complete = false,
        }
    }

    pub(super) fn remove(&mut self, key: RecordKey) {
        if let Some((row, bit)) = place(key)
            && let Some(row) = self.rows.get_mut(row)
        {
            *row &= !bit;
        }
    }

    pub(super) fn lacks(&self, key: RecordKey) -> bool {
        self.complete && !self.holds(key)
    }

    pub(super) fn count(&self) -> u32 {
        self.rows.iter().copied().map(u64::count_ones).sum()
    }

    fn holds(&self, key: RecordKey) -> bool {
        place(key)
            .and_then(|(row, bit)| self.rows.get(row).map(|bits| bits & bit != 0))
            .unwrap_or(false)
    }
}

fn place(key: RecordKey) -> Option<(usize, u64)> {
    let item = key.item();
    if item >= ITEMS {
        return None;
    }
    let row = match key.scope() {
        Scope::Profile(slot) if slot < PROFILE_SLOTS => slot,
        Scope::Linked(slot) if slot < MAX_LINKED => PROFILE_SLOTS + slot,
        Scope::Global => GLOBAL_ROW,
        Scope::Profile(_) | Scope::Linked(_) => return None,
    };
    Some((usize::from(row), 1_u64 << item))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Order;

    #[test]
    fn a_key_is_lacking_only_while_every_stored_key_is_known() {
        let stored = RecordKey::new(Scope::Profile(1), 0x20);
        let other = RecordKey::new(Scope::Global, 0x20);
        let mut keys = StoredKeys::unknown();
        assert!(!keys.lacks(stored));
        keys.start();
        assert!(keys.lacks(stored));
        keys.insert(stored);
        keys.insert(stored);
        assert!(!keys.lacks(stored));
        assert!(keys.lacks(other));
        keys.remove(stored);
        assert!(keys.lacks(stored));
        keys.insert(stored);
        keys.forget();
        assert!(!keys.lacks(stored));
        assert!(!keys.lacks(other));
    }

    #[test]
    fn a_key_outside_the_set_leaves_no_key_lacking_until_the_next_start() {
        for outside in [
            RecordKey::new(Scope::Profile(PROFILE_SLOTS), 0),
            RecordKey::new(Scope::Linked(MAX_LINKED), 0),
            RecordKey::new(Scope::Global, ITEMS),
        ] {
            let other = RecordKey::new(Scope::Profile(1), 0);
            let mut keys = StoredKeys::unknown();
            keys.start();
            assert!(keys.lacks(outside));
            keys.insert(outside);
            assert!(!keys.lacks(other), "{outside:?}");
            assert!(!keys.lacks(outside), "{outside:?}");
            keys.start();
            assert!(keys.lacks(other), "{outside:?}");
        }
    }

    #[test]
    fn every_scope_and_item_of_the_set_is_held_apart_and_counted_once() {
        let last_item = ITEMS - 1;
        let corners = [
            RecordKey::new(Scope::Profile(0), 0),
            RecordKey::new(Scope::Profile(PROFILE_SLOTS - 1), last_item),
            RecordKey::new(Scope::Linked(0), 0),
            RecordKey::new(Scope::Linked(MAX_LINKED - 1), last_item),
            RecordKey::new(Scope::Global, 0),
            RecordKey::new(Scope::Global, last_item),
        ];
        let mut keys = StoredKeys::unknown();
        keys.start();
        for (count, key) in (1..).zip(corners) {
            keys.insert(key);
            keys.insert(key);
            assert_eq!(keys.count(), count);
        }
        for key in corners {
            assert!(!keys.lacks(key), "{key:?}");
        }
        assert!(keys.lacks(RecordKey::new(Scope::Profile(1), last_item)));
        keys.remove(RecordKey::new(Scope::Global, 0));
        assert_eq!(keys.count(), 5);
        assert!(keys.lacks(RecordKey::new(Scope::Global, 0)));
    }

    #[test]
    fn the_set_covers_every_profile_slot_the_order_keeps() {
        assert_eq!(PROFILE_SLOTS, Order::SLOTS);
    }
}
