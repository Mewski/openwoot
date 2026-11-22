use core::{
    cell::RefCell,
    sync::atomic::{AtomicU8, Ordering},
};

use embassy_sync::{
    blocking_mutex::{Mutex, raw::RawMutex},
    signal::Signal,
};

use super::{Parts, Profile};

pub struct Shared<M: RawMutex> {
    profile: Mutex<M, RefCell<Option<Profile>>>,
    changes: AtomicU8,
    profiles: AtomicU8,
    active: AtomicU8,
    requests: Signal<M, u8>,
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
        }
    }

    pub fn publish(&self, profile: Profile, count: u8, active: u8) {
        self.profile.lock(|cell| *cell.borrow_mut() = Some(profile));
        self.profiles.store(count, Ordering::Relaxed);
        self.active.store(active, Ordering::Relaxed);
        self.mark(Parts::ALL.union(Parts::ACTIVATED));
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

    pub fn read<R>(&self, read: impl FnOnce(&Profile) -> R) -> Option<R> {
        self.profile.lock(|cell| cell.borrow().as_ref().map(read))
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
