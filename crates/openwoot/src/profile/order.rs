use heapless::Vec;
use openwoot_protocol::profile::Slot;
use serde::{Deserialize, Serialize};

use crate::storage::{Record, Scope};

const SLOTS: u8 = 16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Order(Vec<u8, { SLOTS as usize }>);

impl Order {
    pub const SLOTS: u8 = SLOTS;

    #[must_use]
    pub fn identity(slots: u8) -> Self {
        let mut order = Vec::new();
        for slot in 0..slots.min(SLOTS) {
            if order.push(slot).is_err() {
                break;
            }
        }
        Self(order)
    }

    #[must_use]
    pub fn storage_slot(&self, slot: u8) -> u8 {
        self.0.get(usize::from(slot)).copied().unwrap_or(slot)
    }

    #[must_use]
    pub fn scope(&self, slot: u8) -> Scope {
        let profile = Slot::new(slot);
        if profile.is_linked() {
            Scope::Linked(profile.index())
        } else {
            Scope::Profile(self.storage_slot(slot))
        }
    }

    #[must_use]
    pub fn slot_of(&self, storage_slot: u8) -> Option<u8> {
        self.0
            .iter()
            .position(|stored| *stored == storage_slot)
            .and_then(|slot| u8::try_from(slot).ok())
    }

    #[must_use]
    pub fn is_permutation_of(&self, slots: u8) -> bool {
        self.0.len() == usize::from(slots) && (0..slots).all(|slot| self.0.contains(&slot))
    }

    pub fn swap(&mut self, first: u8, second: u8) {
        let low = usize::from(first.min(second));
        let high = usize::from(first.max(second));
        if let Some([lower, .., higher]) = self.0.get_mut(low..=high) {
            core::mem::swap(lower, higher);
        }
    }

    pub fn shift(&mut self, from: u8, to: u8) {
        let (from, to) = (usize::from(from), usize::from(to));
        if from < to {
            for slot in from..to {
                self.swap_neighbours(slot);
            }
        } else {
            for slot in (to..from).rev() {
                self.swap_neighbours(slot);
            }
        }
    }

    fn swap_neighbours(&mut self, slot: usize) {
        if let Some([left, right]) = self.0.get_mut(slot..slot + 2) {
            core::mem::swap(left, right);
        }
    }
}

impl Record for Order {
    const ITEM: u8 = 0x17;
    const VERSION: u8 = 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order(slots: &[u8]) -> Order {
        Order(Vec::from_slice(slots).unwrap())
    }

    #[test]
    fn a_fresh_order_keeps_every_profile_in_its_own_storage_slot() {
        let identity = Order::identity(4);
        assert_eq!(identity, order(&[0, 1, 2, 3]));
        assert_eq!(identity.scope(2), Scope::Profile(2));
        assert_eq!(identity.slot_of(3), Some(3));
        assert_eq!(identity.storage_slot(9), 9);
        assert_eq!(identity.slot_of(9), None);
        assert_eq!(Order::identity(u8::MAX), Order::identity(Order::SLOTS));
    }

    #[test]
    fn a_swap_exchanges_the_storage_slots_of_two_profiles() {
        let mut swapped = Order::identity(4);
        swapped.swap(3, 1);
        assert_eq!(swapped, order(&[0, 3, 2, 1]));
        assert_eq!(swapped.scope(1), Scope::Profile(3));
        assert_eq!(swapped.slot_of(1), Some(3));
        swapped.swap(2, 2);
        swapped.swap(0, 9);
        assert_eq!(swapped, order(&[0, 3, 2, 1]));
    }

    #[test]
    fn a_shift_moves_one_profile_and_closes_the_gap_it_leaves() {
        let mut later = Order::identity(4);
        later.shift(0, 2);
        assert_eq!(later, order(&[1, 2, 0, 3]));
        let mut earlier = Order::identity(4);
        earlier.shift(3, 1);
        assert_eq!(earlier, order(&[0, 3, 1, 2]));
        earlier.shift(2, 2);
        assert_eq!(earlier, order(&[0, 3, 1, 2]));
        earlier.shift(1, 7);
        assert_eq!(earlier, order(&[0, 1, 2, 3]));
    }

    #[test]
    fn only_a_permutation_of_every_slot_orders_the_profiles() {
        assert!(order(&[2, 0, 3, 1]).is_permutation_of(4));
        assert!(!order(&[2, 0, 3, 1]).is_permutation_of(5));
        assert!(!order(&[2, 0, 2, 1]).is_permutation_of(4));
        assert!(!order(&[0, 1, 2, 4]).is_permutation_of(4));
        assert!(!order(&[]).is_permutation_of(1));
    }
}
