use core::{
    fmt,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};

use super::{FeatureRequests, HidProtocol};
use crate::{diagnostics::Counter, hid::Leds};

const NO_PLAYER: u8 = u8::MAX;

pub struct HostState {
    keyboard_leds: AtomicU8,
    keyboard_boot_protocol: AtomicBool,
    feature_requests: FeatureRequests,
    configured: AtomicBool,
    configuration_changes: Signal<CriticalSectionRawMutex, bool>,
    suspended: AtomicBool,
    resets: Counter,
    key_reports: AtomicBool,
    gamepad_detection: AtomicBool,
    xinput_player: AtomicU8,
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
            resets: Counter::new(),
            key_reports: AtomicBool::new(true),
            gamepad_detection: AtomicBool::new(false),
            xinput_player: AtomicU8::new(NO_PLAYER),
        }
    }

    pub fn keyboard_leds(&self) -> Leds {
        Leds::from_bits(self.keyboard_leds.load(Ordering::Relaxed))
    }

    pub fn keyboard_protocol(&self) -> HidProtocol {
        if self.keyboard_boot_protocol.load(Ordering::Relaxed) {
            HidProtocol::Boot
        } else {
            HidProtocol::Report
        }
    }

    pub const fn feature_requests(&self) -> &FeatureRequests {
        &self.feature_requests
    }

    pub fn is_active(&self) -> bool {
        self.configured.load(Ordering::Relaxed) && !self.suspended.load(Ordering::Relaxed)
    }

    pub fn is_configured(&self) -> bool {
        self.configured.load(Ordering::Relaxed)
    }

    pub fn is_suspended(&self) -> bool {
        self.suspended.load(Ordering::Relaxed)
    }

    pub fn resets(&self) -> u32 {
        self.resets.value()
    }

    pub fn key_reports(&self) -> bool {
        self.key_reports.load(Ordering::Relaxed)
    }

    pub fn gamepad_detection(&self) -> bool {
        self.gamepad_detection.load(Ordering::Relaxed)
    }

    pub fn xinput_player(&self) -> Option<u8> {
        let player = self.xinput_player.load(Ordering::Relaxed);
        (player != NO_PLAYER).then_some(player)
    }

    pub(crate) fn set_key_reports(&self, enabled: bool) {
        self.key_reports.store(enabled, Ordering::Relaxed);
    }

    pub(crate) fn set_gamepad_detection(&self, enabled: bool) {
        self.gamepad_detection.store(enabled, Ordering::Relaxed);
    }

    pub(crate) fn set_xinput_player(&self, player: u8) {
        self.xinput_player.store(player, Ordering::Relaxed);
    }

    pub(crate) async fn configuration_change(&self) -> bool {
        self.configuration_changes.wait().await
    }

    pub(super) fn count_reset(&self) {
        self.resets.increment();
    }

    pub(super) fn set_keyboard_leds(&self, leds: u8) {
        self.keyboard_leds.store(leds, Ordering::Relaxed);
    }

    pub(super) fn set_keyboard_protocol(&self, protocol: HidProtocol) {
        self.keyboard_boot_protocol
            .store(protocol == HidProtocol::Boot, Ordering::Relaxed);
    }

    pub(super) fn set_configured(&self, configured: bool) {
        self.configured.store(configured, Ordering::Relaxed);
        self.xinput_player.store(NO_PLAYER, Ordering::Relaxed);
        if configured {
            self.key_reports.store(true, Ordering::Relaxed);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuring_turns_key_reports_back_on_and_forgets_the_player() {
        let host = HostState::new();
        host.set_key_reports(false);
        host.set_xinput_player(2);
        assert!(!host.key_reports());
        assert_eq!(host.xinput_player(), Some(2));
        host.set_configured(true);
        assert!(host.key_reports());
        assert_eq!(host.xinput_player(), None);
    }

    #[test]
    fn deconfiguring_keeps_key_reports_and_detection_as_set() {
        let host = HostState::new();
        host.set_key_reports(false);
        host.set_gamepad_detection(true);
        host.set_xinput_player(1);
        host.set_configured(false);
        assert!(!host.key_reports());
        assert!(host.gamepad_detection());
        assert_eq!(host.xinput_player(), None);
    }
}
