use heapless::Vec;

use super::record::RecordKey;

const CAPACITY: usize = 96;

#[derive(Debug)]
pub(super) struct StoredKeys {
    keys: Vec<RecordKey, CAPACITY>,
    complete: bool,
}

impl StoredKeys {
    pub(super) const fn unknown() -> Self {
        Self {
            keys: Vec::new(),
            complete: false,
        }
    }

    pub(super) fn forget(&mut self) {
        self.keys.clear();
        self.complete = false;
    }

    pub(super) fn start(&mut self) {
        self.keys.clear();
        self.complete = true;
    }

    pub(super) fn insert(&mut self, key: RecordKey) {
        if !self.keys.contains(&key) && self.keys.push(key).is_err() {
            self.complete = false;
        }
    }

    pub(super) fn remove(&mut self, key: RecordKey) {
        self.keys.retain(|stored| *stored != key);
    }

    pub(super) fn lacks(&self, key: RecordKey) -> bool {
        self.complete && !self.keys.contains(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Scope;

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
    fn more_keys_than_fit_leave_no_key_lacking_until_the_next_start() {
        let mut keys = StoredKeys::unknown();
        keys.start();
        for item in 0..=u8::MAX {
            keys.insert(RecordKey::new(Scope::Profile(0), item));
        }
        assert!(!keys.lacks(RecordKey::new(Scope::Profile(1), 0)));
        keys.start();
        assert!(keys.lacks(RecordKey::new(Scope::Profile(1), 0)));
    }
}
