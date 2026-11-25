use std::collections::HashSet;

use openwoot::keys::{Behaviour, Entry, Key, ModTap};
use openwoot_protocol::{
    advanced::{Interrupt, Resolution, STROKES, Stroke, StrokeEvent},
    keys::{Action, Layer},
};

use super::support::{COLUMNS, Engine, KEY_A, KEY_D, KEY_S, ROWS, engine, step};

const SHIFT: u8 = 0xE1;
const CAPS: Key = Key::new(3, 0);

struct Sensors(HashSet<Key>);

impl Sensors {
    fn synced(engine: &mut Engine) -> Self {
        let mut sensors = Self(HashSet::new());
        sensors.sync(engine);
        sensors
    }

    fn sync(&mut self, engine: &mut Engine) {
        engine.take_hold_changes(|key, holds| {
            if holds {
                self.0.insert(key);
            } else {
                self.0.remove(&key);
            }
        });
        for row in (0u8..).take(ROWS) {
            for column in (0u8..).take(COLUMNS) {
                let key = Key::new(row, column);
                assert_eq!(self.0.contains(&key), engine.holds_events(key), "{key:?}");
            }
        }
    }
}

fn run(engine: &mut Engine, passes: &[(&[(Key, u16)], u16)]) {
    let mut sensors = Sensors::synced(engine);
    for (events, elapsed_ms) in passes {
        step(engine, events, *elapsed_ms);
        sensors.sync(engine);
    }
}

fn with_entries(keys: &[(Key, Action)], entries: &[Entry]) -> Engine {
    let mut engine = engine(keys);
    engine.set_advanced(entries.iter().copied());
    engine
}

fn shift_or_a() -> Entry {
    Entry {
        key: KEY_A,
        layer: 0,
        behaviour: Behaviour::ModTap(ModTap {
            tap: Action::Keyboard(0x04),
            hold: Action::Keyboard(SHIFT),
            hold_ms: 200,
            interrupt: Interrupt::Auto,
        }),
    }
}

#[test]
fn reported_hold_changes_follow_plain_presses_and_releases() {
    let mut keys = engine(&[
        (KEY_A, Action::Keyboard(0x04)),
        (KEY_S, Action::Keyboard(0x16)),
    ]);
    run(
        &mut keys,
        &[
            (&[(KEY_A, 2000)], 10),
            (&[(KEY_A, 2000), (KEY_S, 1500)], 10),
            (&[(KEY_A, 400), (KEY_S, 2000)], 10),
            (&[(KEY_A, 0), (KEY_S, 0)], 10),
            (&[], 10),
        ],
    );
}

#[test]
fn reported_hold_changes_follow_a_mod_tap_decided_by_another_key() {
    let mut keys = with_entries(
        &[(KEY_A, Action::None), (KEY_S, Action::Keyboard(0x16))],
        &[shift_or_a()],
    );
    run(
        &mut keys,
        &[
            (&[(KEY_A, 2000)], 10),
            (&[(KEY_A, 2000), (KEY_S, 2000)], 10),
            (&[(KEY_A, 2000), (KEY_S, 0)], 10),
            (&[(KEY_A, 0)], 10),
            (&[(KEY_A, 0)], 300),
            (&[], 10),
        ],
    );
}

#[test]
fn reported_hold_changes_follow_toggles_strokes_and_pairs() {
    let mut strokes = [Stroke::default(); STROKES];
    strokes[0] = Stroke {
        action: Action::Keyboard(0x05),
        on_press: StrokeEvent::Tap,
        ..Stroke::default()
    };
    let mut keys = with_entries(
        &[
            (CAPS, Action::None),
            (KEY_A, Action::Keyboard(0x04)),
            (KEY_S, Action::None),
            (KEY_D, Action::Keyboard(0x07)),
        ],
        &[
            Entry {
                key: CAPS,
                layer: 0,
                behaviour: Behaviour::Toggle(Action::Layer(Layer::One)),
            },
            Entry {
                key: KEY_S,
                layer: 0,
                behaviour: Behaviour::DynamicKeystroke {
                    strokes,
                    bottom_out: None,
                },
            },
            Entry {
                key: KEY_A,
                layer: 0,
                behaviour: Behaviour::Socd {
                    other: KEY_D,
                    resolution: Resolution::LastPressed,
                    both_when_bottomed_out: false,
                },
            },
        ],
    );
    run(
        &mut keys,
        &[
            (&[(CAPS, 2000)], 10),
            (&[(CAPS, 0)], 10),
            (&[(KEY_S, 2000)], 10),
            (&[(KEY_S, 0)], 10),
            (&[(KEY_A, 2000)], 1),
            (&[(KEY_A, 2000), (KEY_D, 2000)], 1),
            (&[(KEY_A, 0), (KEY_D, 2000)], 1),
            (&[(KEY_D, 0)], 1),
            (&[(CAPS, 2000)], 10),
            (&[(CAPS, 0)], 10),
            (&[], 10),
        ],
    );
}

#[test]
fn reported_hold_changes_cover_key_resets_new_bindings_and_key_reports_going_off() {
    let mut keys = with_entries(&[(KEY_A, Action::None)], &[shift_or_a()]);
    let mut sensors = Sensors::synced(&mut keys);
    step(&mut keys, &[(KEY_A, 2000)], 10);
    sensors.sync(&mut keys);
    keys.set_key_reports(false);
    step(&mut keys, &[(KEY_A, 2000)], 10);
    sensors.sync(&mut keys);
    keys.set_key_reports(true);
    step(&mut keys, &[(KEY_A, 2000)], 10);
    sensors.sync(&mut keys);
    keys.reset_keys();
    sensors.sync(&mut keys);
    keys.set_advanced([]);
    sensors.sync(&mut keys);
    step(&mut keys, &[(KEY_A, 0)], 10);
    sensors.sync(&mut keys);
}

#[test]
fn a_pass_without_events_reports_no_hold_change() {
    let mut keys = engine(&[(KEY_A, Action::Keyboard(0x04))]);
    keys.take_hold_changes(|_, _| {});
    step(&mut keys, &[], 10);
    let mut reported = 0;
    keys.take_hold_changes(|_, _| reported += 1);
    assert_eq!(reported, 0);
}
