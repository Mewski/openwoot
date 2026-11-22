use core::{
    fmt,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

use super::FeatureRequests;
use crate::hid::Leds;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Protocol {
    Boot,
    #[default]
    Report,
}

pub struct HostState {
    keyboard_leds: AtomicU8,
    keyboard_boot_protocol: AtomicBool,
    feature_requests: FeatureRequests,
    configured: AtomicBool,
    configuration_changes: Signal<CriticalSectionRawMutex, bool>,
    suspended: AtomicBool,
}

impl HostState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            keyboard_leds: AtomicU8::new(0),
            keyboard_boot_protocol: AtomicBool::new(false),
            feature_requests: FeatureRequests::new(),
            configured: AtomicBool::new(false),
            configuration_changes: Signal::new(),
            suspended: AtomicBool::new(false),
        }
    }

    pub fn keyboard_leds(&self) -> Leds {
        Leds::from_bits(self.keyboard_leds.load(Ordering::Relaxed))
    }

    pub fn keyboard_protocol(&self) -> Protocol {
        if self.keyboard_boot_protocol.load(Ordering::Relaxed) {
            Protocol::Boot
        } else {
            Protocol::Report
        }
    }

    pub const fn feature_requests(&self) -> &FeatureRequests {
        &self.feature_requests
    }

    pub fn is_active(&self) -> bool {
        self.configured.load(Ordering::Relaxed) && !self.suspended.load(Ordering::Relaxed)
    }

    pub(crate) async fn configuration_change(&self) -> bool {
        self.configuration_changes.wait().await
    }

    pub(super) fn set_keyboard_leds(&self, leds: u8) {
        self.keyboard_leds.store(leds, Ordering::Relaxed);
    }

    pub(super) fn set_keyboard_protocol(&self, protocol: Protocol) {
        self.keyboard_boot_protocol
            .store(protocol == Protocol::Boot, Ordering::Relaxed);
    }

    pub(super) fn set_configured(&self, configured: bool) {
        self.configured.store(configured, Ordering::Relaxed);
        self.configuration_changes.signal(configured);
    }

    pub(super) fn set_suspended(&self, suspended: bool) {
        self.suspended.store(suspended, Ordering::Relaxed);
    }
}

impl Default for HostState {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for HostState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HostState")
            .field("keyboard_leds", &self.keyboard_leds)
            .field("keyboard_boot_protocol", &self.keyboard_boot_protocol)
            .field("feature_requests", &self.feature_requests)
            .field("configured", &self.configured)
            .field("suspended", &self.suspended)
            .finish_non_exhaustive()
    }
}
