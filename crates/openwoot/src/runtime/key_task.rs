use embassy_futures::select::{Either, select};
use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_time::{Duration, Instant, Timer};
use openwoot_protocol::keys::ScanMode;

use super::{KeyNotices, KeyReports, LedWindow};
use crate::{
    analog::SwitchModel,
    diagnostics::Diagnostics,
    keys::{Functions, Keys, Notice, Parts, ProfileSource, Shared, Switch},
    lighting::{KeyActivity, KeyEvent, KeyEvents, LockState},
    scan::{self, Sensing, SensorReadout},
    usb::{Face, HidProtocol, HostState, KeyboardFormat},
};

const CALIBRATION_TIMEOUT: Duration = Duration::from_secs(1);
const CYCLE_TIMEOUT: Duration = Duration::from_millis(50);
const RETRY_DELAY: Duration = Duration::from_millis(1);
const SWITCH_DEPENDENT: Parts = Parts::SETTINGS
    .union(Parts::OVERRIDES)
    .union(Parts::ADVANCED);

pub struct KeyTask<'d, M: RawMutex> {
    pub shared: &'d Shared<M>,
    pub reports: &'d KeyReports<M>,
    pub events: &'d KeyEvents,
    pub notices: &'d KeyNotices,
    pub host: &'d HostState,
    pub sensors: &'d SensorReadout,
    pub face: Face,
    pub diagnostics: &'d Diagnostics,
    pub led_window: &'d LedWindow,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    last_ms: Option<u32>,
    locks: Option<LockState>,
    rebound_filter: Option<bool>,
    suspended: bool,
    factory_switch: Option<SwitchModel>,
    switches_synced: bool,
    resets: Option<u32>,
}

impl<M: RawMutex> KeyTask<'_, M> {
    pub const IDLE_EVERY: Duration = Duration::from_millis(1);
    pub const IDLE_FOR: Duration = Duration::from_micros(100);

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
        loop {
            match select(scan.calibrate(), Timer::after(CALIBRATION_TIMEOUT)).await {
                Either::First(true) => break,
                Either::First(false) => Timer::after(RETRY_DELAY).await,
                Either::Second(()) => self.recover::<S, INPUTS, ADDRESSES, ROWS, COLUMNS>(scan),
            }
            self.led_window.grant().await;
        }
        let mut progress = Progress::default();
        let mut started = None;
        let mut idle_at = Instant::now() + Self::IDLE_EVERY;
        loop {
            let cycle_ms = if let Some(cycle_ms) = started.take() {
                cycle_ms
            } else {
                let cycle_ms = now_ms();
                if !scan.begin(cycle_ms) {
                    Timer::after(RETRY_DELAY).await;
                    self.led_window.grant().await;
                    continue;
                }
                cycle_ms
            };
            match select(scan.finished(), Timer::after(CYCLE_TIMEOUT)).await {
                Either::First(()) => {
                    self.diagnostics.scan_cycle_completed();
                    let passed = self.pass(scan, keys, &mut progress, cycle_ms);
                    let lights_wait = self.led_window.is_wanted();
                    let single =
                        scan.with(|scanner| scanner.mode()) == Some(Some(scan::Mode::Single));
                    if !lights_wait && single {
                        let next_ms = now_ms();
                        started = scan.begin(next_ms).then_some(next_ms);
                    }
                    if passed {
                        self.deliver(keys, &mut progress, cycle_ms);
                    }
                    if lights_wait {
                        self.led_window.grant().await;
                    } else if !single && Instant::now() >= idle_at {
                        Timer::after(Self::IDLE_FOR).await;
                        idle_at = Instant::now() + Self::IDLE_EVERY;
                    }
                }
                Either::Second(()) => {
                    self.recover::<S, INPUTS, ADDRESSES, ROWS, COLUMNS>(scan);
                    self.led_window.grant().await;
                }
            }
        }
    }

    pub fn pass<
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
    ) -> bool
    where
        S: Sensing,
        F: Functions,
    {
        let elapsed_ms = progress
            .last_ms
            .map_or(0, |last_ms| now_ms.wrapping_sub(last_ms));
        keys.set_boot_format(
            self.face.keyboard == KeyboardFormat::Boot
                || self.host.keyboard_protocol() == HidProtocol::Boot,
        );
        keys.set_key_reports(self.host.key_reports());
        keys.set_gamepad_detection(self.host.gamepad_detection());
        keys.set_xinput_player(self.host.xinput_player());
        let travel = scan.travel();
        let resets = self.host.resets();
        let reset = progress
            .resets
            .replace(resets)
            .is_some_and(|seen| seen != resets);
        if reset || self.host.is_suspended() {
            keys.stop_macro();
        }
        let passed = scan.with(|scanner| {
            let switched = self.fit_switches::<S, INPUTS, ADDRESSES, COLUMNS>(scanner, progress);
            if switched || !progress.switches_synced {
                for (position, key) in scanner.map().iter() {
                    if let Some(sensor) = scanner.sensor(position) {
                        keys.set_switch(key, Switch::from_switch(sensor.switch()));
                    }
                }
                progress.switches_synced = true;
            }
            let mut parts = self.shared.take_changes();
            if switched {
                parts = parts.union(SWITCH_DEPENDENT);
            }
            if !parts.is_empty() {
                keys.configure(self.shared, parts);
                let scan_mode = self.shared.read(|profile| profile.settings.scan_mode);
                if parts.contains(Parts::SETTINGS)
                    && let Some(scan_mode) = scan_mode
                {
                    scanner.request_mode(match scan_mode {
                        ScanMode::Single => scan::Mode::Single,
                        ScanMode::Parallel => scan::Mode::Parallel,
                    });
                }
                if parts.contains(Parts::ACTIVATED) || parts.contains(Parts::PROFILES) {
                    let (count, active) = self.shared.profiles();
                    let (selected, returning) = self.shared.links();
                    keys.set_profiles(count, active);
                    keys.link_profiles(selected, returning);
                    if parts.contains(Parts::ACTIVATED) {
                        self.diagnostics.profile_applied(active);
                    }
                }
                if parts.contains(Parts::KEYMAPS) || parts.contains(Parts::ADVANCED) {
                    for (indicator, key) in keys.indicator_keys() {
                        self.notices.send(Notice::IndicatorKey { indicator, key });
                    }
                }
            }
            let rebound_filter = self.shared.rebound_filter();
            if progress.rebound_filter != Some(rebound_filter) {
                scanner.set_rebound_filter(rebound_filter);
                progress.rebound_filter = Some(rebound_filter);
            }
            self.sensors.serve::<ROWS, COLUMNS, _, _>(scanner, travel);
            if scanner.is_settling() {
                return false;
            }
            keys.pass(
                scanner
                    .events()
                    .iter()
                    .map(|event| (event.key, event.travel)),
                u16::try_from(elapsed_ms).unwrap_or(u16::MAX),
                |key| {
                    scanner
                        .key_position(key)
                        .map_or(0, |position| travel.get(position))
                },
            );
            keys.play(self.shared.macros(), self.reports.drained());
            keys.take_hold_changes(|key, holds| scanner.hold_events(key, holds));
            true
        }) == Some(true);
        if passed {
            progress.last_ms = Some(now_ms);
            self.diagnostics.keys_passed();
        }
        passed
    }

    fn fit_switches<
        S: Sensing,
        const INPUTS: usize,
        const ADDRESSES: usize,
        const COLUMNS: usize,
    >(
        &self,
        scanner: &mut scan::Scanner<S, INPUTS, ADDRESSES>,
        progress: &mut Progress,
    ) -> bool {
        if !self.shared.take_switch_change() {
            return false;
        }
        let factory = *progress.factory_switch.get_or_insert_with(|| {
            scanner
                .map()
                .iter()
                .next()
                .and_then(|(position, _)| scanner.sensor(position))
                .map_or(SwitchModel::DEFAULT, |sensor| sensor.switch().kind())
        });
        scanner.apply_switches(|key| self.shared.fitted_switch(key.index(COLUMNS), factory));
        true
    }

    pub fn deliver<F: Functions, const ROWS: usize, const COLUMNS: usize>(
        &self,
        keys: &mut Keys<F, ROWS, COLUMNS>,
        progress: &mut Progress,
        now_ms: u32,
    ) {
        let suspended = self.host.is_suspended();
        if progress.suspended != suspended {
            progress.suspended = suspended;
            self.reports.resend();
        }
        if !suspended && self.reports.publish(keys.output()) {
            self.diagnostics.key_report_published();
        }
        let mut pressed = false;
        for notice in keys.take_notices() {
            match notice {
                Notice::Key { key, event } => {
                    pressed |= event == KeyEvent::Pressed;
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
        if suspended && pressed {
            self.reports.request_wake_up();
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
            self.diagnostics.profile_requested();
            self.shared.request_profile(profile);
        }
    }

    fn recover<
        S: Sensing,
        const INPUTS: usize,
        const ADDRESSES: usize,
        const ROWS: usize,
        const COLUMNS: usize,
    >(
        &self,
        scan: &scan::Shared<S, INPUTS, ADDRESSES>,
    ) {
        scan.recover();
        self.diagnostics.scan_recovered();
        scan.with(|scanner| {
            self.sensors
                .serve::<ROWS, COLUMNS, _, _>(scanner, scan.travel());
        });
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
