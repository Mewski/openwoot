use std::{
    pin::pin,
    task::{Context, Poll, Waker},
};

use embassy_futures::block_on;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, MockDriver};
use openwoot::{
    diagnostics::{Diagnostics, Hardware},
    keys::Key,
    lighting::{HostControl, KeyActivity, KeyEvent, KeyEvents, LedMatrix, Lighting},
    profile,
    runtime::{self, LedWindow},
    usb::HostState,
};
use openwoot_protocol::{
    diagnostics::Usb,
    lighting::{Effects, Indicators, Settings},
};

use super::support::{COLUMNS, GEOMETRY, LENGTH, MAP, PAGES, ROWS, VARIANT, effect};
use crate::support::{between_scans, hold_clock};

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
    failing_shut_down: bool,
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
        if !running && self.failing_shut_down {
            Err(())
        } else {
            Ok(())
        }
    }

    async fn write(&mut self, bank: u8, first_register: u8, values: &[u8]) -> Result<(), ()> {
        self.calls
            .push(Call::Write(bank, first_register, values.len()));
        if self.failing_writes { Err(()) } else { Ok(()) }
    }
}

type Engine = Lighting<(), ROWS, COLUMNS, PAGES, LENGTH>;

fn settings(brightness: u8, effects: Effects) -> Settings {
    let white = openwoot_protocol::lighting::Color {
        red: 255,
        green: 255,
        blue: 255,
        alpha: 255,
    };
    Settings {
        brightness,
        indicators: Indicators {
            caps_lock: white,
            scroll_lock: white,
            num_lock: white,
            layer_lock: white,
        },
        effects,
    }
}

fn engine(effects: Effects) -> Engine {
    let mut lighting = Lighting::new(&MAP, &VARIANT, GEOMETRY, (), 0);
    lighting.apply_settings(&settings(255, effects), 0);
    lighting
}

fn writes(calls: &[Call]) -> usize {
    calls
        .iter()
        .filter(|call| matches!(call, Call::Write(..)))
        .count()
}

#[test]
fn the_leds_wake_with_usb_and_every_page_is_sent_once() {
    let mut lighting = engine(Effects::new());
    let events = KeyEvents::new();
    let host = HostState::new();
    let diagnostics = Diagnostics::new();
    let window = LedWindow::new();
    let mut task = runtime::Lighting {
        lighting: &mut lighting,
        events: &events,
        notices: &runtime::KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &diagnostics,
        profile: &profile::Shared::<CriticalSectionRawMutex>::new(),
        led_window: &window,
    };
    let active = || {
        diagnostics
            .report(Usb::default(), Hardware::default())
            .lighting
            .active
    };
    let mut matrix = Recorder::default();
    between_scans(&window, task.step(&mut matrix, &(), 0, false));
    assert_eq!(matrix.calls, Vec::new());
    between_scans(&window, task.step(&mut matrix, &(), 1, true));
    assert_eq!(matrix.calls.first(), Some(&Call::Running(true)));
    assert_eq!(writes(&matrix.calls), PAGES);
    assert!(active());
    assert!(matrix.calls.contains(&Call::Write(1, 0, LENGTH)));
    matrix.calls.clear();
    between_scans(&window, task.step(&mut matrix, &(), 2, false));
    assert_eq!(matrix.calls, [Call::Running(false)]);
    assert!(!active());
}

#[test]
fn key_presses_reach_the_effects_through_the_event_channel() {
    let mut effects = Effects::new();
    effects.push(effect(11, [40, 0, 0, 0])).unwrap();
    let mut lighting = engine(effects);
    let events = KeyEvents::new();
    let host = HostState::new();
    let window = LedWindow::new();
    let mut task = runtime::Lighting {
        lighting: &mut lighting,
        events: &events,
        notices: &runtime::KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &Diagnostics::new(),
        profile: &profile::Shared::<CriticalSectionRawMutex>::new(),
        led_window: &window,
    };
    let mut matrix = Recorder::default();
    for now_ms in 0..40 {
        between_scans(&window, task.step(&mut matrix, &(), now_ms, true));
    }
    let key = Key::new(2, 2);
    events
        .try_send(KeyActivity {
            key,
            event: KeyEvent::Pressed,
            at_ms: 40,
        })
        .unwrap();
    between_scans(&window, task.step(&mut matrix, &(), 40, true));
    between_scans(&window, task.step(&mut matrix, &(), 56, true));
    let color = task.lighting.key_color(key);
    assert_eq!(color.map(|color| color.green), Some(255));
}

#[test]
fn a_failed_write_sends_every_page_again() {
    let mut lighting = engine(Effects::new());
    let events = KeyEvents::new();
    let host = HostState::new();
    let diagnostics = Diagnostics::new();
    let window = LedWindow::new();
    let mut task = runtime::Lighting {
        lighting: &mut lighting,
        events: &events,
        notices: &runtime::KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &diagnostics,
        profile: &profile::Shared::<CriticalSectionRawMutex>::new(),
        led_window: &window,
    };
    let mut matrix = Recorder {
        failing_writes: true,
        ..Recorder::default()
    };
    assert!(!between_scans(
        &window,
        task.step(&mut matrix, &(), 1, true)
    ));
    assert_eq!(writes(&matrix.calls), 1);
    matrix.failing_writes = false;
    matrix.calls.clear();
    assert!(between_scans(&window, task.step(&mut matrix, &(), 2, true)));
    assert_eq!(writes(&matrix.calls), PAGES);
    let report = diagnostics
        .report(Usb::default(), Hardware::default())
        .lighting;
    assert_eq!(
        (report.frames, report.page_writes, report.failed_page_writes),
        (2, u32::try_from(PAGES).unwrap(), 1)
    );
}

#[test]
fn a_page_the_driver_already_holds_is_not_sent_again() {
    let mut effects = Effects::new();
    effects.push(effect(1, [0, 4, 1, 0])).unwrap();
    let mut lighting = Engine::new(&MAP, &VARIANT, GEOMETRY, (), 0);
    lighting.apply_settings(&settings(0, effects.clone()), 0);
    let events = KeyEvents::new();
    let host = HostState::new();
    let diagnostics = Diagnostics::new();
    let window = LedWindow::new();
    let mut task = runtime::Lighting {
        lighting: &mut lighting,
        events: &events,
        notices: &runtime::KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &diagnostics,
        profile: &profile::Shared::<CriticalSectionRawMutex>::new(),
        led_window: &window,
    };
    let mut matrix = Recorder::default();
    assert!(between_scans(&window, task.step(&mut matrix, &(), 1, true)));
    assert_eq!(writes(&matrix.calls), PAGES);
    let first_frame = task.lighting.frame().clone();
    matrix.calls.clear();
    for now_ms in 2..400 {
        assert!(between_scans(
            &window,
            task.step(&mut matrix, &(), now_ms, true)
        ));
    }
    assert_ne!(
        task.lighting.frame(),
        &first_frame,
        "the dark effect stood still"
    );
    assert_eq!(
        matrix.calls,
        [],
        "dark frames went out to a driver that holds them"
    );
    task.lighting.apply_settings(&settings(255, effects), 400);
    matrix.failing_writes = true;
    assert!(!between_scans(
        &window,
        task.step(&mut matrix, &(), 400, true)
    ));
    matrix.failing_writes = false;
    matrix.calls.clear();
    assert!(between_scans(
        &window,
        task.step(&mut matrix, &(), 401, true)
    ));
    assert_eq!(
        writes(&matrix.calls),
        PAGES,
        "a failed write leaves pages the driver may not hold"
    );
    let report = diagnostics
        .report(Usb::default(), Hardware::default())
        .lighting;
    assert_eq!(
        (report.page_writes, report.failed_page_writes),
        (2 * u32::try_from(PAGES).unwrap(), 1)
    );
}

#[test]
fn page_writes_waiting_for_the_scan_give_way_to_the_next_frame() {
    let _clock = hold_clock();
    let clock = MockDriver::get();
    let mut effects = Effects::new();
    effects.push(effect(1, [0, 4, 1, 0])).unwrap();
    let mut lighting = engine(effects);
    let events = KeyEvents::new();
    let host = HostState::new();
    let window = LedWindow::new();
    let mut task = runtime::Lighting {
        lighting: &mut lighting,
        events: &events,
        notices: &runtime::KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &Diagnostics::new(),
        profile: &profile::Shared::<CriticalSectionRawMutex>::new(),
        led_window: &window,
    };
    let mut context = Context::from_waker(Waker::noop());
    let mut matrix = Recorder::default();
    {
        let mut step = pin!(task.step(&mut matrix, &(), 1, true));
        while step.as_mut().poll(&mut context).is_pending() {
            let _ = pin!(window.grant()).poll(&mut context);
        }
    }
    assert_eq!(writes(&matrix.calls), PAGES);
    matrix.calls.clear();
    task.lighting.invalidate();
    {
        let mut step = pin!(task.step(&mut matrix, &(), 17, true));
        assert!(step.as_mut().poll(&mut context).is_pending());
        clock.advance(Duration::from_millis(13));
        assert!(step.as_mut().poll(&mut context).is_pending());
        clock.advance(Duration::from_millis(1));
        assert_eq!(
            step.as_mut().poll(&mut context),
            Poll::Ready(true),
            "the pages held the frame back"
        );
    }
    assert_eq!(task.lighting.until_frame_ms(17), 16);
    assert_eq!(matrix.calls, [], "a page went out without a gap");
    {
        let mut step = pin!(task.step(&mut matrix, &(), 33, true));
        while step.as_mut().poll(&mut context).is_pending() {
            let _ = pin!(window.grant()).poll(&mut context);
        }
    }
    assert_eq!(
        task.lighting.until_frame_ms(33),
        16,
        "the next frame waited"
    );
    assert_eq!(
        writes(&matrix.calls),
        PAGES,
        "the pages left behind did not follow the next frame"
    );
}

#[test]
fn frames_that_change_every_page_still_send_each_page_within_two_frames() {
    let _clock = hold_clock();
    let clock = MockDriver::get();
    let mut effects = Effects::new();
    effects.push(effect(1, [0, 4, 1, 0])).unwrap();
    let mut lighting = engine(effects);
    let events = KeyEvents::new();
    let host = HostState::new();
    let window = LedWindow::new();
    let mut task = runtime::Lighting {
        lighting: &mut lighting,
        events: &events,
        notices: &runtime::KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &Diagnostics::new(),
        profile: &profile::Shared::<CriticalSectionRawMutex>::new(),
        led_window: &window,
    };
    let mut context = Context::from_waker(Waker::noop());
    let mut matrix = Recorder::default();
    {
        let mut step = pin!(task.step(&mut matrix, &(), 1, true));
        while step.as_mut().poll(&mut context).is_pending() {
            let _ = pin!(window.grant()).poll(&mut context);
        }
    }
    let page_of = |call: &Call| match *call {
        Call::Write(bank, first_register, _) => (0..PAGES).find(|page| {
            MAP.page(*page).is_some_and(|layout| {
                (layout.bank, layout.first_register) == (bank, first_register)
            })
        }),
        Call::Initialize | Call::Running(_) => None,
    };
    let mut sent = Vec::new();
    for frame in 1..=3 * PAGES {
        task.lighting.invalidate();
        matrix.calls.clear();
        {
            let now_ms = 1 + 16 * u32::try_from(frame).unwrap();
            let mut step = pin!(task.step(&mut matrix, &(), now_ms, true));
            let mut granted = 0;
            while step.as_mut().poll(&mut context).is_pending() {
                if granted + 1 < PAGES {
                    let _ = pin!(window.grant()).poll(&mut context);
                    granted += 1;
                } else {
                    clock.advance(Duration::from_millis(14));
                }
            }
        }
        let pages: Vec<usize> = matrix.calls.iter().filter_map(page_of).collect();
        assert_eq!(pages.len(), PAGES - 1, "frame {frame}: {pages:?}");
        sent.push(pages);
    }
    let order = sent.concat();
    assert!(
        order
            .windows(2)
            .all(|pair| pair[1] == (pair[0] + 1) % PAGES),
        "the pages did not go out in turn: {sent:?}"
    );
    for (frame, pair) in sent.windows(2).enumerate() {
        for page in 0..PAGES {
            assert!(
                pair.iter().any(|pages| pages.contains(&page)),
                "page {page} waited through frames {} and {}: {sent:?}",
                frame + 1,
                frame + 2
            );
        }
    }
}

#[test]
fn a_late_initialisation_wakes_the_reset_chip_and_sends_every_page() {
    let mut lighting = engine(Effects::new());
    let events = KeyEvents::new();
    let host = HostState::new();
    let window = LedWindow::new();
    let mut task = runtime::Lighting {
        lighting: &mut lighting,
        events: &events,
        notices: &runtime::KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &Diagnostics::new(),
        profile: &profile::Shared::<CriticalSectionRawMutex>::new(),
        led_window: &window,
    };
    let mut matrix = Recorder::default();
    between_scans(&window, task.step(&mut matrix, &(), 1, true));
    matrix.calls.clear();
    assert!(block_on(task.initialize(&mut matrix)));
    assert!(between_scans(&window, task.step(&mut matrix, &(), 2, true)));
    assert_eq!(
        matrix.calls.get(..2),
        Some(&[Call::Initialize, Call::Running(true)][..])
    );
    assert_eq!(writes(&matrix.calls), PAGES);
}

#[test]
fn a_failed_shut_down_is_retried_until_the_chip_stops() {
    let mut lighting = engine(Effects::new());
    let events = KeyEvents::new();
    let host = HostState::new();
    let window = LedWindow::new();
    let mut task = runtime::Lighting {
        lighting: &mut lighting,
        events: &events,
        notices: &runtime::KeyNotices::new(),
        host: &host,
        host_control: &HostControl::new(),
        diagnostics: &Diagnostics::new(),
        profile: &profile::Shared::<CriticalSectionRawMutex>::new(),
        led_window: &window,
    };
    let mut matrix = Recorder {
        failing_shut_down: true,
        ..Recorder::default()
    };
    between_scans(&window, task.step(&mut matrix, &(), 1, true));
    matrix.calls.clear();
    for now_ms in 2..6 {
        assert!(!between_scans(
            &window,
            task.step(&mut matrix, &(), now_ms, false)
        ));
    }
    assert_eq!(matrix.calls, vec![Call::Running(false); 4]);
    assert!(task.lighting.output_enabled());
    matrix.failing_shut_down = false;
    matrix.calls.clear();
    for now_ms in 6..10 {
        assert!(between_scans(
            &window,
            task.step(&mut matrix, &(), now_ms, false)
        ));
    }
    assert_eq!(matrix.calls, [Call::Running(false)]);
    assert!(!task.lighting.output_enabled());
}
