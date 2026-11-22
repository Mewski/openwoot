use embassy_futures::yield_now;
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_time::Instant;
use openwoot_protocol::keys::ScanMode;

use super::{KeyNotices, KeyReports};
use crate::{
    keys::{Functions, Keys, Notice, Parts, Shared, Switch},
    lighting::{KeyActivity, KeyEvents, LockState},
    scan::{self, Sensing},
    usb::{Face, HidProtocol, HostState, KeyboardFormat},
};

pub struct KeyTask<'d, M: RawMutex> {
    pub shared: &'d Shared<M>,
    pub reports: &'d KeyReports<M>,
    pub events: &'d KeyEvents,
    pub notices: &'d KeyNotices,
    pub host: &'d HostState,
    pub face: Face,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    last_ms: Option<u32>,
    locks: Option<LockState>,
}

impl<M: RawMutex> KeyTask<'_, M> {
    pub async fn run<
        S,
        F,
        const INPUTS: usize,
        const ADDRESSES: usize,
        const ROWS: usize,
        const COLUMNS: usize,
    >(
        &self,
        scan: &scan::Shared<S, INPUTS, ADDRESSES>,
        keys: &mut Keys<F, ROWS, COLUMNS>,
    ) -> !
    where
        S: Sensing,
        F: Functions,
    {
        keys.set_mode(self.face.mode);
        while !scan.calibrate().await {
            yield_now().await;
        }
        let mut progress = Progress::default();
        loop {
            let now_ms = now_ms();
            if scan.cycle(now_ms).await {
                self.step(scan, keys, &mut progress, now_ms);
            } else {
                yield_now().await;
            }
        }
    }

    pub fn step<
        S,
        F,
        const INPUTS: usize,
        const ADDRESSES: usize,
        const ROWS: usize,
        const COLUMNS: usize,
    >(
        &self,
        scan: &scan::Shared<S, INPUTS, ADDRESSES>,
        keys: &mut Keys<F, ROWS, COLUMNS>,
        progress: &mut Progress,
        now_ms: u32,
    ) where
        S: Sensing,
        F: Functions,
    {
        let elapsed_ms = progress
            .last_ms
            .map_or(0, |last_ms| now_ms.wrapping_sub(last_ms));
        progress.last_ms = Some(now_ms);
        keys.set_boot_format(
            self.face.keyboard == KeyboardFormat::Boot
                || self.host.keyboard_protocol() == HidProtocol::Boot,
        );
        keys.set_player(self.reports.player());
        let travel = scan.travel();
        let processed = scan.with(|scanner| {
            let map = *scanner.map();
            for (position, key) in map.iter() {
                if let Some(sensor) = scanner.sensor(position) {
                    keys.set_switch(key, Switch::from_switch(sensor.switch()));
                }
            }
            let parts = self.shared.take_changes();
            if !parts.is_empty() {
                let scan_mode = self.shared.read(|profile| {
                    keys.configure(profile, parts);
                    profile.settings.scan_mode
                });
                if parts.contains(Parts::SETTINGS)
                    && let Some(scan_mode) = scan_mode
                {
                    scanner.request_mode(match scan_mode {
                        ScanMode::Single => scan::Mode::Single,
                        ScanMode::Parallel => scan::Mode::Parallel,
                    });
                }
                if parts.contains(Parts::ACTIVATED) {
                    let (count, active) = self.shared.profiles();
                    keys.set_profiles(count, active);
                }
                if parts.contains(Parts::KEYMAPS) || parts.contains(Parts::ADVANCED) {
                    for (indicator, key) in keys.indicator_keys() {
                        self.notices.send(Notice::IndicatorKey { indicator, key });
                    }
                }
            }
            keys.pass(
                scanner
                    .events()
                    .iter()
                    .map(|event| (event.key, event.travel)),
                u16::try_from(elapsed_ms & u32::from(u16::MAX)).unwrap_or(u16::MAX),
                |key| map.position(key).map_or(0, |position| travel.get(position)),
            );
            for (position, key) in map.iter() {
                let holds = keys.holds_events(key);
                if let Some(sensor) = scanner.sensor_mut(position) {
                    sensor.hold_events(holds);
                }
            }
        });
        if processed.is_none() {
            return;
        }
        self.reports.publish(keys.output());
        for notice in keys.take_notices() {
            match notice {
                Notice::Key { key, event } => {
                    let _ = self.events.try_send(KeyActivity {
                        key,
                        event,
                        at_ms: now_ms,
                    });
                }
                notice => {
                    self.notices.send(notice);
                }
            }
        }
        let leds = self.host.keyboard_leds();
        let locks = LockState {
            caps_lock: leds.caps_lock(),
            scroll_lock: leds.scroll_lock(),
            num_lock: leds.num_lock(),
            layer_locked: keys.layers().locked() != 0,
        };
        if progress.locks != Some(locks) && self.notices.send(Notice::Locks(locks)) {
            progress.locks = Some(locks);
        }
        if let Some(profile) = keys.take_profile_request() {
            self.shared.request_profile(profile);
        }
    }
}

impl<M: RawMutex> core::fmt::Debug for KeyTask<'_, M> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("KeyTask")
            .field("face", &self.face)
            .finish_non_exhaustive()
    }
}

fn now_ms() -> u32 {
    u32::try_from(Instant::now().as_millis() & u64::from(u32::MAX)).unwrap_or_default()
}
