use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

use embassy_sync::{
    blocking_mutex::{Mutex, raw::RawMutex},
    signal::Signal,
};

use super::{Parts, Profile};

pub trait ProfileSource {
    fn read<R>(&self, read: impl FnOnce(&Profile) -> R) -> Option<R>;
}

pub struct Shared<M: RawMutex> {
    profile: Mutex<M, RefCell<Option<Profile>>>,
    changes: AtomicU8,
    profiles: AtomicU8,
    active: AtomicU8,
    requests: Signal<M, u8>,
    rebound_filter: AtomicBool,
}

impl<M: RawMutex> Shared<M> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            profile: Mutex::new(RefCell::new(None)),
            changes: AtomicU8::new(0),
            profiles: AtomicU8::new(1),
            active: AtomicU8::new(0),
            requests: Signal::new(),
            rebound_filter: AtomicBool::new(false),
        }
    }

    pub fn edit<R>(&self, edit: impl FnOnce(&mut Profile) -> R) -> R {
        self.profile
            .lock(|cell| edit(cell.borrow_mut().get_or_insert_with(Profile::empty)))
    }

    pub fn activate(&self, count: u8, active: u8) {
        self.profiles.store(count, Ordering::Relaxed);
        self.active.store(active, Ordering::Relaxed);
        self.mark(Parts::ALL.union(Parts::ACTIVATED));
    }

    pub fn set_profiles(&self, count: u8, active: u8) {
        self.profiles.store(count, Ordering::Relaxed);
        self.active.store(active, Ordering::Relaxed);
        self.mark(Parts::PROFILES);
    }

    pub fn set_rebound_filter(&self, enabled: bool) {
        self.rebound_filter.store(enabled, Ordering::Relaxed);
    }

    #[must_use]
    pub fn rebound_filter(&self) -> bool {
        self.rebound_filter.load(Ordering::Relaxed)
    }

    pub fn change<R>(&self, parts: Parts, change: impl FnOnce(&mut Profile) -> R) -> Option<R> {
        let result = self
            .profile
            .lock(|cell| cell.borrow_mut().as_mut().map(change));
        if result.is_some() {
            self.mark(parts);
        }
        result
    }

    #[must_use]
    pub fn take_changes(&self) -> Parts {
        Parts::from_bits(self.changes.swap(0, Ordering::AcqRel))
    }

    #[must_use]
    pub fn profiles(&self) -> (u8, u8) {
        (
            self.profiles.load(Ordering::Relaxed),
            self.active.load(Ordering::Relaxed),
        )
    }

    pub fn request_profile(&self, profile: u8) {
        self.requests.signal(profile);
    }

    pub async fn profile_request(&self) -> u8 {
        self.requests.wait().await
    }

    fn mark(&self, parts: Parts) {
        self.changes.fetch_or(parts.bits(), Ordering::AcqRel);
    }
}

impl<M: RawMutex> ProfileSource for Shared<M> {
    fn read<R>(&self, read: impl FnOnce(&Profile) -> R) -> Option<R> {
        self.profile
            .lock(|cell| cell.try_borrow().ok()?.as_ref().map(read))
    }
}

impl<M: RawMutex> Default for Shared<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: RawMutex> core::fmt::Debug for Shared<M> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Shared")
            .field("changes", &self.changes)
            .field("profiles", &self.profiles)
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use embassy_sync::blocking_mutex::raw::NoopRawMutex;

    use super::*;

    #[test]
    fn a_read_inside_an_edit_finds_no_profile_instead_of_panicking() {
        let shared = Shared::<NoopRawMutex>::new();
        assert_eq!(shared.edit(|_| shared.read(|_| ())), None);
        assert_eq!(shared.read(|_| ()), Some(()));
    }
}
