use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use openwoot_protocol::diagnostics::{Keys, Lighting, Profile, Report, Scan, Storage, Usb};

use super::{Counter, Hardware};

const NO_PROFILE: u8 = u8::MAX;

#[derive(Debug)]
pub struct Diagnostics {
    scan_cycles: Counter,
    scan_recoveries: Counter,
    key_passes: Counter,
    key_reports: Counter,
    profile: AtomicU8,
    profile_requests: Counter,
    profile_switches: Counter,
    lighting_initialized: AtomicBool,
    lighting_active: AtomicBool,
    frames: Counter,
    page_writes: Counter,
    failed_page_writes: Counter,
    storage_loads: Counter,
    storage_stores: Counter,
    storage_failures: Counter,
}

impl Diagnostics {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scan_cycles: Counter::new(),
            scan_recoveries: Counter::new(),
            key_passes: Counter::new(),
            key_reports: Counter::new(),
            profile: AtomicU8::new(NO_PROFILE),
            profile_requests: Counter::new(),
            profile_switches: Counter::new(),
            lighting_initialized: AtomicBool::new(false),
            lighting_active: AtomicBool::new(false),
            frames: Counter::new(),
            page_writes: Counter::new(),
            failed_page_writes: Counter::new(),
            storage_loads: Counter::new(),
            storage_stores: Counter::new(),
            storage_failures: Counter::new(),
        }
    }

    pub fn scan_cycle_completed(&self) {
        self.scan_cycles.increment();
    }

    pub fn scan_recovered(&self) {
        self.scan_recoveries.increment();
    }

    pub fn keys_passed(&self) {
        self.key_passes.increment();
    }

    pub fn key_report_published(&self) {
        self.key_reports.increment();
    }

    pub fn profile_requested(&self) {
        self.profile_requests.increment();
    }

    pub fn profile_applied(&self, profile: u8) {
        let previous = self.profile.swap(profile, Ordering::Relaxed);
        if previous != NO_PROFILE && previous != profile {
            self.profile_switches.increment();
        }
    }

    pub fn set_lighting_initialized(&self, initialized: bool) {
        self.lighting_initialized
            .store(initialized, Ordering::Relaxed);
    }

    pub fn set_lighting_active(&self, active: bool) {
        self.lighting_active.store(active, Ordering::Relaxed);
    }

    pub fn frame_rendered(&self) {
        self.frames.increment();
    }

    pub fn page_written<T, E>(&self, result: &Result<T, E>) {
        match result {
            Ok(_) => self.page_writes.increment(),
            Err(_) => self.failed_page_writes.increment(),
        }
    }

    pub fn storage_loaded<T, E>(&self, result: &Result<T, E>) {
        self.storage_access(&self.storage_loads, result);
    }

    pub fn storage_stored<T, E>(&self, result: &Result<T, E>) {
        self.storage_access(&self.storage_stores, result);
    }

    #[must_use]
    pub fn report(&self, usb: Usb, hardware: Hardware) -> Report {
        let profile = self.profile.load(Ordering::Relaxed);
        Report {
            scan: Scan {
                cycles: self.scan_cycles.value(),
                recoveries: self.scan_recoveries.value(),
                overruns: hardware.scan_overruns,
            },
            keys: Keys {
                passes: self.key_passes.value(),
                reports: self.key_reports.value(),
            },
            profile: Profile {
                active: (profile != NO_PROFILE).then_some(profile),
                requests: self.profile_requests.value(),
                switches: self.profile_switches.value(),
            },
            lighting: Lighting {
                initialized: self.lighting_initialized.load(Ordering::Relaxed),
                active: self.lighting_active.load(Ordering::Relaxed),
                frames: self.frames.value(),
                page_writes: self.page_writes.value(),
                failed_page_writes: self.failed_page_writes.value(),
            },
            led_bus: hardware.led_bus,
            storage: Storage {
                loads: self.storage_loads.value(),
                stores: self.storage_stores.value(),
                failures: self.storage_failures.value(),
            },
            flash_bus: hardware.flash_bus,
            usb,
            stack: hardware.stack,
        }
    }

    fn storage_access<T, E>(&self, done: &Counter, result: &Result<T, E>) {
        match result {
            Ok(_) => done.increment(),
            Err(_) => self.storage_failures.increment(),
        }
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_switch_counts_only_a_change_after_the_first_profile() {
        let diagnostics = Diagnostics::new();
        let active = |diagnostics: &Diagnostics| {
            diagnostics
                .report(Usb::default(), Hardware::default())
                .profile
        };
        assert_eq!(active(&diagnostics).active, None);
        diagnostics.profile_applied(2);
        diagnostics.profile_applied(2);
        assert_eq!(
            active(&diagnostics),
            Profile {
                active: Some(2),
                requests: 0,
                switches: 0,
            }
        );
        diagnostics.profile_requested();
        diagnostics.profile_applied(0);
        assert_eq!(
            active(&diagnostics),
            Profile {
                active: Some(0),
                requests: 1,
                switches: 1,
            }
        );
    }

    #[test]
    fn failed_storage_accesses_count_apart_from_loads_and_stores() {
        let diagnostics = Diagnostics::new();
        diagnostics.storage_loaded::<(), ()>(&Ok(()));
        diagnostics.storage_loaded::<(), ()>(&Err(()));
        diagnostics.storage_stored::<(), ()>(&Ok(()));
        diagnostics.storage_stored::<(), ()>(&Ok(()));
        assert_eq!(
            diagnostics
                .report(Usb::default(), Hardware::default())
                .storage,
            Storage {
                loads: 1,
                stores: 2,
                failures: 1,
            }
        );
    }
}
