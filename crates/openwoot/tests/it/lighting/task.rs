use embassy_futures::block_on;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use openwoot::{
    diagnostics::{Diagnostics, Hardware},
    keys::Key,
    lighting::{HostControl, KeyActivity, KeyEvent, KeyEvents, LedMatrix, Lighting},
    profile, runtime,
    usb::HostState,
};
use openwoot_protocol::{
    diagnostics::Usb,
    lighting::{Effects, Indicators, Settings},
};

use super::support::{COLUMNS, GEOMETRY, LENGTH, MAP, PAGES, ROWS, VARIANT, effect};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Call {
    Initialize,
    Running(bool),
    Write(u8, u8, usize),
}

#[derive(Default)]
struct Recorder {
    calls: Vec<Call>,
    failing_writes: bool,
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "the recorder completes every call at once"
)]
impl LedMatrix for Recorder {
    type Error = ();

    async fn initialize(&mut self) -> Result<(), ()> {
        self.calls.push(Call::Initialize);
        Ok(())
    }

    async fn set_running(&mut self, running: bool) -> Result<(), ()> {
        self.calls.push(Call::Running(running));
        Ok(())
    }

    async fn write(&mut self, bank: u8, first_register: u8, values: &[u8]) -> Result<(), ()> {
        self.calls
            .push(Call::Write(bank, first_register, values.len()));
        if self.failing_writes { Err(()) } else { Ok(()) }
    }
}

type Shared = Mutex<CriticalSectionRawMutex, Lighting<(), ROWS, COLUMNS, PAGES, LENGTH>>;

fn shared(effects: Effects) -> Shared {
    let mut lighting = Lighting::new(&MAP, &VARIANT, GEOMETRY, (), 0);
    let white = openwoot_protocol::lighting::Color {
        red: 255,
        green: 255,
        blue: 255,
        alpha: 255,
    };
    lighting.apply_settings(
        &Settings {
            brightness: 255,
            indicators: Indicators {
                caps_lock: white,
                scroll_lock: white,
                num_lock: white,
                layer_lock: white,
            },
            effects,
        },
        0,
    );
    Mutex::new(lighting)
}

fn writes(calls: &[Call]) -> usize {
    calls
        .iter()
        .filter(|call| matches!(call, Call::Write(..)))
        .count()
}

#[test]
fn the_leds_wake_with_usb_and_every_page_is_sent_once() {
    let lighting = shared(Effects::new());
    let events = KeyEvents::new();
    let host = HostState::new();
    let diagnostics = Diagnostics::new();
    let task = runtime::Lighting {
        lighting: &lighting,
        events: &events,
        notices: &runtime::KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &diagnostics,
        profile: &profile::Shared::new(),
    };
    let active = || {
        diagnostics
            .report(Usb::default(), Hardware::default())
            .lighting
            .active
    };
    let mut matrix = Recorder::default();
    block_on(task.step(&mut matrix, &(), 0, false));
    assert_eq!(matrix.calls, Vec::new());
    block_on(task.step(&mut matrix, &(), 1, true));
    assert_eq!(matrix.calls.first(), Some(&Call::Running(true)));
    assert_eq!(writes(&matrix.calls), PAGES);
    assert!(active());
    assert!(matrix.calls.contains(&Call::Write(1, 0, LENGTH)));
    matrix.calls.clear();
    block_on(task.step(&mut matrix, &(), 2, false));
    assert_eq!(matrix.calls, [Call::Running(false)]);
    assert!(!active());
}

#[test]
fn key_presses_reach_the_effects_through_the_event_channel() {
    let mut effects = Effects::new();
    effects.push(effect(11, [40, 0, 0, 0])).unwrap();
    let lighting = shared(effects);
    let events = KeyEvents::new();
    let host = HostState::new();
    let task = runtime::Lighting {
        lighting: &lighting,
        events: &events,
        notices: &runtime::KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &Diagnostics::new(),
        profile: &profile::Shared::new(),
    };
    let mut matrix = Recorder::default();
    for now_ms in 0..40 {
        block_on(task.step(&mut matrix, &(), now_ms, true));
    }
    let key = Key::new(2, 2);
    events
        .try_send(KeyActivity {
            key,
            event: KeyEvent::Pressed,
            at_ms: 40,
        })
        .unwrap();
    block_on(task.step(&mut matrix, &(), 40, true));
    block_on(task.step(&mut matrix, &(), 56, true));
    let color = block_on(lighting.lock()).key_color(key);
    assert_eq!(color.map(|color| color.green), Some(255));
}

#[test]
fn a_failed_write_sends_every_page_again() {
    let lighting = shared(Effects::new());
    let events = KeyEvents::new();
    let host = HostState::new();
    let diagnostics = Diagnostics::new();
    let task = runtime::Lighting {
        lighting: &lighting,
        events: &events,
        notices: &runtime::KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &diagnostics,
        profile: &profile::Shared::new(),
    };
    let mut matrix = Recorder {
        failing_writes: true,
        ..Recorder::default()
    };
    block_on(task.step(&mut matrix, &(), 1, true));
    assert_eq!(writes(&matrix.calls), 1);
    matrix.failing_writes = false;
    matrix.calls.clear();
    block_on(task.step(&mut matrix, &(), 2, true));
    assert_eq!(writes(&matrix.calls), PAGES);
    let report = diagnostics
        .report(Usb::default(), Hardware::default())
        .lighting;
    assert_eq!(
        (report.frames, report.page_writes, report.failed_page_writes),
        (2, u32::try_from(PAGES).unwrap(), 1)
    );
}
