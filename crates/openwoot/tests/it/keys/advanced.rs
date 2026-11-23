use openwoot::keys::{Behaviour, Entry, Key, ModTap, Notice};
use openwoot_protocol::{
    advanced::{Interrupt, Resolution, STROKES, Stroke, StrokeEvent},
    keys::{Action, Function, Layer},
};

use super::support::{Engine, KEY_A, KEY_D, KEY_S, engine, step};

const SHIFT: u8 = 0xE1;
const CAPS: Key = Key::new(3, 0);
const FN: Key = Key::new(5, 12);

fn with_entry(keys: &[(Key, Action)], behaviour: Behaviour) -> Engine {
    let mut engine = engine(keys);
    engine.set_advanced([Entry {
        key: KEY_A,
        layer: 0,
        behaviour,
    }]);
    engine
}

fn outputs(engine: &mut Engine, passes: &[(&[(Key, u16)], u16)]) -> Vec<Vec<u8>> {
    passes
        .iter()
        .map(|(events, elapsed_ms)| step(engine, events, *elapsed_ms))
        .collect()
}

#[test]
fn a_dynamic_keystroke_taps_at_the_actuation_point_and_holds_from_the_bottom_out() {
    let mut strokes = [Stroke::default(); STROKES];
    strokes[0] = Stroke {
        action: Action::Keyboard(0x04),
        on_press: StrokeEvent::Tap,
        ..Stroke::default()
    };
    strokes[1] = Stroke {
        action: Action::Keyboard(0x05),
        on_bottom_out: StrokeEvent::Press,
        on_lift: StrokeEvent::Release,
        ..Stroke::default()
    };
    let mut keys = with_entry(
        &[(KEY_A, Action::None)],
        Behaviour::DynamicKeystroke {
            strokes,
            bottom_out: None,
        },
    );
    let mut passes: Vec<(&[(Key, u16)], u16)> = vec![(&[(KEY_A, 1200)], 5); 6];
    passes.extend([(&[(KEY_A, 3800)][..], 5)]);
    passes.extend(vec![(&[(KEY_A, 3000)][..], 5); 5]);
    let got = outputs(&mut keys, &passes);
    let tap: Vec<u8> = vec![0x04];
    let held: Vec<u8> = vec![0x05];
    assert_eq!(
        got,
        [
            tap.clone(),
            tap.clone(),
            tap.clone(),
            tap.clone(),
            tap,
            vec![],
            held.clone(),
            held.clone(),
            held.clone(),
            held.clone(),
            held,
            vec![]
        ]
    );
}

fn shift_or_a() -> Behaviour {
    Behaviour::ModTap(ModTap {
        tap: Action::Keyboard(0x04),
        hold: Action::Keyboard(SHIFT),
        hold_ms: 200,
        interrupt: Interrupt::Auto,
    })
}

#[test]
fn a_mod_tap_taps_when_released_early_and_holds_after_the_hold_time() {
    let mut tapped = with_entry(&[(KEY_A, Action::None)], shift_or_a());
    let got = outputs(
        &mut tapped,
        &[
            (&[(KEY_A, 2000)], 10),
            (&[(KEY_A, 2000)], 50),
            (&[(KEY_A, 0)], 10),
            (&[(KEY_A, 0)], 10),
            (&[(KEY_A, 0)], 10),
            (&[(KEY_A, 0)], 10),
        ],
    );
    assert_eq!(
        got,
        [vec![], vec![], vec![0x04], vec![0x04], vec![0x04], vec![]]
    );
    let mut held = with_entry(&[(KEY_A, Action::None)], shift_or_a());
    let got = outputs(
        &mut held,
        &[(&[(KEY_A, 2000)], 10), (&[(KEY_A, 2000)], 200)],
    );
    assert_eq!(got, [vec![], vec![SHIFT]]);
}

#[test]
fn another_key_press_decides_a_waiting_mod_tap_and_comes_one_pass_later() {
    let mut keys = with_entry(
        &[(KEY_A, Action::None), (KEY_S, Action::Keyboard(0x16))],
        shift_or_a(),
    );
    let got = outputs(
        &mut keys,
        &[
            (&[(KEY_A, 2000)], 10),
            (&[(KEY_A, 2000), (KEY_S, 2000)], 10),
            (&[(KEY_A, 2000), (KEY_S, 2000)], 10),
        ],
    );
    assert_eq!(got, [vec![], vec![SHIFT], vec![0x16, SHIFT]]);
}

#[test]
fn a_toggle_latches_on_a_short_press_and_not_on_a_long_one() {
    let mut keys = with_entry(
        &[(KEY_A, Action::None)],
        Behaviour::Toggle(Action::Keyboard(0x04)),
    );
    let got = outputs(
        &mut keys,
        &[
            (&[(KEY_A, 2000)], 10),
            (&[(KEY_A, 0)], 50),
            (&[(KEY_A, 0)], 10),
            (&[(KEY_A, 2000)], 10),
            (&[(KEY_A, 0)], 10),
            (&[(KEY_A, 2000)], 10),
            (&[(KEY_A, 2000)], 300),
            (&[(KEY_A, 0)], 10),
        ],
    );
    assert_eq!(
        got,
        [
            vec![0x04],
            vec![0x04],
            vec![0x04],
            vec![0x04],
            vec![],
            vec![0x04],
            vec![0x04],
            vec![]
        ]
    );
}

#[test]
fn snap_tap_reports_the_key_pressed_last() {
    let mut keys = with_entry(
        &[
            (KEY_A, Action::Keyboard(0x04)),
            (KEY_D, Action::Keyboard(0x07)),
        ],
        Behaviour::Socd {
            other: KEY_D,
            resolution: Resolution::LastPressed,
            both_when_bottomed_out: false,
        },
    );
    let got = outputs(
        &mut keys,
        &[
            (&[(KEY_A, 2000)], 1),
            (&[(KEY_A, 2000), (KEY_D, 2000)], 1),
            (&[(KEY_A, 2000), (KEY_D, 0)], 1),
        ],
    );
    assert_eq!(got, [vec![0x04], vec![0x07], vec![0x04]]);
}

#[test]
fn depth_priority_reports_the_deeper_key_and_keeps_its_decision_at_similar_depth() {
    let mut keys = with_entry(
        &[
            (KEY_A, Action::Keyboard(0x04)),
            (KEY_D, Action::Keyboard(0x07)),
        ],
        Behaviour::DepthPriority {
            other: KEY_D,
            both_bottomed_out: Resolution::Both,
        },
    );
    let got = outputs(
        &mut keys,
        &[
            (&[(KEY_A, 2000), (KEY_D, 1000)], 1),
            (&[(KEY_A, 2000), (KEY_D, 2100)], 1),
            (&[(KEY_A, 2000), (KEY_D, 2500)], 1),
        ],
    );
    assert_eq!(got, [vec![0x04], vec![0x04], vec![0x07]]);
}

fn layer_or_caps_lock() -> Entry {
    Entry {
        key: CAPS,
        layer: 0,
        behaviour: Behaviour::ModTap(ModTap {
            tap: Action::Keyboard(0x39),
            hold: Action::Layer(Layer::One),
            hold_ms: 200,
            interrupt: Interrupt::Auto,
        }),
    }
}

fn control_or_b() -> Behaviour {
    Behaviour::ModTap(ModTap {
        tap: Action::Keyboard(0x05),
        hold: Action::Keyboard(0xE0),
        hold_ms: 200,
        interrupt: Interrupt::Auto,
    })
}

fn last_pressed_of(other: Key) -> Behaviour {
    Behaviour::Socd {
        other,
        resolution: Resolution::LastPressed,
        both_when_bottomed_out: false,
    }
}

#[test]
fn a_key_that_decides_a_layer_tap_acts_only_on_the_new_layer() {
    let mut keys = engine(&[
        (CAPS, Action::None),
        (KEY_A, Action::Keyboard(0x04)),
        (KEY_D, Action::Keyboard(0x07)),
    ]);
    keys.set_action(KEY_A, Layer::One, Action::Keyboard(0x50));
    keys.set_advanced([
        layer_or_caps_lock(),
        Entry {
            key: KEY_A,
            layer: 0,
            behaviour: last_pressed_of(KEY_D),
        },
    ]);
    let got = outputs(
        &mut keys,
        &[
            (&[(CAPS, 2000)], 10),
            (&[(CAPS, 2000), (KEY_A, 2000)], 10),
            (&[(CAPS, 2000), (KEY_A, 2000)], 10),
            (&[(CAPS, 2000), (KEY_A, 0)], 10),
        ],
    );
    assert_eq!(got, [vec![], vec![], vec![0x50], vec![]]);
}

#[test]
fn a_dynamic_keystroke_of_the_old_layer_does_not_start_when_its_press_decides_a_layer_tap() {
    let mut strokes = [Stroke::default(); STROKES];
    strokes[0] = Stroke {
        action: Action::Keyboard(0x04),
        on_press: StrokeEvent::Press,
        on_release: StrokeEvent::Release,
        ..Stroke::default()
    };
    let mut keys = engine(&[(CAPS, Action::None), (KEY_A, Action::None)]);
    keys.set_action(KEY_A, Layer::One, Action::Keyboard(0x50));
    keys.set_advanced([
        layer_or_caps_lock(),
        Entry {
            key: KEY_A,
            layer: 0,
            behaviour: Behaviour::DynamicKeystroke {
                strokes,
                bottom_out: None,
            },
        },
    ]);
    let got = outputs(
        &mut keys,
        &[
            (&[(CAPS, 2000)], 10),
            (&[(CAPS, 2000), (KEY_A, 2000)], 10),
            (&[(CAPS, 2000), (KEY_A, 2000)], 10),
            (&[(CAPS, 2000), (KEY_A, 0)], 10),
            (&[(CAPS, 2000)], 10),
        ],
    );
    assert_eq!(got, [vec![], vec![], vec![0x50], vec![], vec![]]);
    assert!(!keys.holds_events(KEY_A));
}

#[test]
fn a_mod_tap_pressed_again_during_its_tap_can_still_hold() {
    let mut keys = with_entry(&[(KEY_A, Action::None)], shift_or_a());
    let got = outputs(
        &mut keys,
        &[
            (&[(KEY_A, 2000)], 10),
            (&[(KEY_A, 0)], 5),
            (&[(KEY_A, 2000)], 5),
            (&[(KEY_A, 2000)], 100),
            (&[(KEY_A, 2000)], 100),
            (&[(KEY_A, 0)], 10),
        ],
    );
    assert_eq!(
        got,
        [vec![], vec![0x04], vec![], vec![], vec![SHIFT], vec![]]
    );
}

#[test]
fn a_plain_key_on_the_active_layer_decides_a_waiting_mod_tap() {
    let mut keys = engine(&[
        (FN, Action::Layer(Layer::One)),
        (KEY_S, Action::None),
        (KEY_D, Action::None),
    ]);
    keys.set_action(KEY_D, Layer::One, Action::Keyboard(0x07));
    keys.set_advanced([
        Entry {
            key: KEY_S,
            layer: 1,
            behaviour: shift_or_a(),
        },
        Entry {
            key: KEY_D,
            layer: 0,
            behaviour: control_or_b(),
        },
    ]);
    let got = outputs(
        &mut keys,
        &[
            (&[(FN, 2000)], 10),
            (&[(FN, 2000), (KEY_S, 2000)], 10),
            (&[(FN, 2000), (KEY_S, 2000), (KEY_D, 2000)], 10),
            (&[(FN, 2000), (KEY_S, 2000), (KEY_D, 2000)], 10),
        ],
    );
    assert_eq!(got, [vec![], vec![], vec![SHIFT], vec![0x07, SHIFT]]);
}

#[test]
fn a_mod_tap_released_within_one_pass_taps_and_does_not_hold_forever() {
    let mut keys = engine(&[
        (FN, Action::Layer(Layer::One)),
        (KEY_S, Action::None),
        (KEY_D, Action::Keyboard(0x07)),
    ]);
    keys.set_advanced([
        Entry {
            key: KEY_S,
            layer: 1,
            behaviour: shift_or_a(),
        },
        Entry {
            key: KEY_D,
            layer: 1,
            behaviour: control_or_b(),
        },
    ]);
    let got = outputs(
        &mut keys,
        &[
            (&[(FN, 2000)], 1),
            (&[(FN, 2000), (KEY_S, 2000)], 1),
            (&[(FN, 2000), (KEY_S, 2000), (KEY_D, 2000)], 1),
            (&[(FN, 2000), (KEY_S, 2000), (KEY_D, 0)], 1),
            (&[(FN, 0), (KEY_S, 0), (KEY_D, 0)], 1),
        ],
    );
    assert_eq!(got, [vec![], vec![], vec![], vec![0x05], vec![0x04, 0x05]]);
    for _ in 0..100 {
        step(&mut keys, &[(KEY_S, 0), (KEY_D, 0)], 1);
    }
    assert_eq!(step(&mut keys, &[(KEY_S, 0), (KEY_D, 0)], 1), []);
    assert!(!keys.holds_events(KEY_D));
}

#[test]
fn a_disabled_key_stays_silent_as_half_of_a_pair() {
    let mut keys = engine(&[
        (KEY_A, Action::Keyboard(0x04)),
        (KEY_D, Action::Keyboard(0x07)),
        (FN, Action::Layer(Layer::One)),
    ]);
    keys.set_action(
        KEY_A,
        Layer::One,
        Action::Function(Function::ToggleKeyDisable),
    );
    keys.set_advanced([Entry {
        key: KEY_A,
        layer: 0,
        behaviour: last_pressed_of(KEY_D),
    }]);
    step(&mut keys, &[(FN, 2000)], 1);
    step(&mut keys, &[(FN, 2000), (KEY_A, 2000)], 1);
    step(&mut keys, &[(FN, 0), (KEY_A, 0)], 1);
    step(&mut keys, &[(KEY_A, 0)], 1);
    assert_eq!(step(&mut keys, &[(KEY_A, 2000)], 1), []);
    assert_eq!(step(&mut keys, &[(KEY_A, 2000), (KEY_D, 2000)], 1), [0x07]);
}

#[test]
fn advanced_keys_rest_while_key_reports_are_off() {
    let mut keys = with_entry(
        &[(KEY_A, Action::None)],
        Behaviour::Toggle(Action::Keyboard(0x04)),
    );
    keys.set_key_reports(false);
    step(&mut keys, &[(KEY_A, 2000)], 10);
    assert_eq!(keys.output().key_states().entries().len(), 0);
    step(&mut keys, &[(KEY_A, 0)], 10);
    assert!(
        !keys
            .take_notices()
            .iter()
            .any(|notice| matches!(notice, Notice::Toggle { latched: true, .. }))
    );
    keys.set_key_reports(true);
    assert_eq!(step(&mut keys, &[], 10), []);
    assert_eq!(step(&mut keys, &[(KEY_A, 2000)], 10), [0x04]);
}

#[test]
fn a_toggle_latched_before_key_reports_go_off_is_let_go() {
    let mut keys = with_entry(
        &[(KEY_A, Action::None)],
        Behaviour::Toggle(Action::Keyboard(0x04)),
    );
    step(&mut keys, &[(KEY_A, 2000)], 10);
    assert_eq!(step(&mut keys, &[(KEY_A, 0)], 10), [0x04]);
    keys.set_key_reports(false);
    step(&mut keys, &[(KEY_A, 0)], 10);
    assert!(keys.take_notices().contains(&Notice::Toggle {
        key: KEY_A,
        latched: false,
        layer_key: false,
    }));
    keys.set_key_reports(true);
    assert_eq!(step(&mut keys, &[(KEY_A, 0)], 10), []);
    assert!(!keys.holds_events(KEY_A));
}

#[test]
fn a_key_held_while_it_becomes_a_toggle_does_not_latch_on_release() {
    let mut keys = engine(&[(KEY_A, Action::Keyboard(0x1D))]);
    step(&mut keys, &[(KEY_A, 2000)], 10);
    keys.set_advanced([Entry {
        key: KEY_A,
        layer: 0,
        behaviour: Behaviour::Toggle(Action::Keyboard(0x04)),
    }]);
    assert_eq!(step(&mut keys, &[(KEY_A, 0)], 10), []);
    assert!(!keys.holds_events(KEY_A));
}

#[test]
fn a_toggle_on_the_first_key_of_the_matrix_announces_its_latch() {
    let first = Key::new(0, 0);
    let mut keys = engine(&[(first, Action::None)]);
    keys.set_advanced([Entry {
        key: first,
        layer: 0,
        behaviour: Behaviour::Toggle(Action::Keyboard(0x04)),
    }]);
    step(&mut keys, &[(first, 2000)], 1);
    step(&mut keys, &[(first, 0)], 1);
    assert!(keys.take_notices().contains(&Notice::Toggle {
        key: first,
        latched: true,
        layer_key: false,
    }));
}

#[test]
fn depth_priority_gives_the_key_pressed_last_when_both_go_down_together() {
    let mut keys = with_entry(
        &[
            (KEY_A, Action::Keyboard(0x04)),
            (KEY_D, Action::Keyboard(0x07)),
        ],
        Behaviour::DepthPriority {
            other: KEY_D,
            both_bottomed_out: Resolution::Both,
        },
    );
    let got = outputs(
        &mut keys,
        &[
            (&[(KEY_A, 2000), (KEY_D, 2050)], 1),
            (&[(KEY_A, 2500), (KEY_D, 2550)], 1),
        ],
    );
    assert_eq!(got, [vec![0x07], vec![0x07]]);
}

#[test]
fn a_reset_of_the_keys_ends_a_held_mod_tap_of_another_layer() {
    let mut keys = engine(&[(FN, Action::Layer(Layer::One)), (KEY_S, Action::None)]);
    keys.set_advanced([Entry {
        key: KEY_S,
        layer: 1,
        behaviour: shift_or_a(),
    }]);
    let held = outputs(
        &mut keys,
        &[
            (&[(FN, 2000)], 10),
            (&[(FN, 2000), (KEY_S, 2000)], 10),
            (&[(FN, 2000), (KEY_S, 2000)], 300),
        ],
    );
    assert_eq!(held.last(), Some(&vec![SHIFT]));
    keys.reset_keys();
    assert_eq!(step(&mut keys, &[(KEY_S, 0)], 10), []);
    assert!(!keys.holds_events(KEY_S));
}

#[test]
fn a_mod_tap_beyond_the_waiting_slots_is_left_to_its_own_timer() {
    let waiting: Vec<Key> = (0..6).map(|column| Key::new(2, column)).collect();
    let mut bindings: Vec<(Key, Action)> = waiting.iter().map(|key| (*key, Action::None)).collect();
    bindings.push((KEY_S, Action::Keyboard(0x16)));
    let mut keys = engine(&bindings);
    keys.set_advanced(waiting.iter().zip(0xE0..).map(|(key, modifier)| Entry {
        key: *key,
        layer: 0,
        behaviour: Behaviour::ModTap(ModTap {
            tap: Action::Keyboard(0x04),
            hold: Action::Keyboard(modifier),
            hold_ms: 200,
            interrupt: Interrupt::Auto,
        }),
    }));
    let held: Vec<(Key, u16)> = waiting.iter().map(|key| (*key, 2000)).collect();
    let mut with_s = held.clone();
    with_s.push((KEY_S, 2000));
    let got = outputs(
        &mut keys,
        &[
            (held.as_slice(), 10),
            (with_s.as_slice(), 10),
            (with_s.as_slice(), 10),
        ],
    );
    assert_eq!(
        got,
        [
            vec![],
            vec![0xE0, 0xE1, 0xE2, 0xE3, 0xE4],
            vec![0x16, 0xE0, 0xE1, 0xE2, 0xE3, 0xE4]
        ]
    );
}
