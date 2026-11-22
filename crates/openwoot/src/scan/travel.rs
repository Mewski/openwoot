use core::{
    marker::PhantomData,
    sync::atomic::{AtomicU16, Ordering},
};

use super::{ADDRESSES_PER_INPUT, Grid, Position};

#[derive(Debug)]
pub struct Travel<const INPUTS: usize, const ADDRESSES: usize> {
    keys: Grid<AtomicU16, INPUTS>,
    addresses: PhantomData<[(); ADDRESSES]>,
}

impl<const INPUTS: usize, const ADDRESSES: usize> Default for Travel<INPUTS, ADDRESSES> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const INPUTS: usize, const ADDRESSES: usize> Travel<INPUTS, ADDRESSES> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            keys: Grid::new([const { [const { AtomicU16::new(0) }; ADDRESSES_PER_INPUT] }; INPUTS]),
            addresses: PhantomData,
        }
    }

    #[must_use]
    pub fn get(&self, position: Position) -> u16 {
        self.keys
            .get(position)
            .map_or(0, |travel| travel.load(Ordering::Relaxed))
    }

    #[inline(always)]
    pub(super) fn set(&self, position: Position, travel: u16) {
        if let Some(slot) = self.keys.get(position) {
            slot.store(travel, Ordering::Relaxed);
        }
    }
}
