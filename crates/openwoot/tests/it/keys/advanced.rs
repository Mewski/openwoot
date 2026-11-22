use openwoot::keys::{Behaviour, Entry, Key, ModTap};
use openwoot_protocol::{
    advanced::{Interrupt, Resolution, STROKES, Stroke, StrokeEvent},
    keys::Action,
};

use super::support::{Engine, KEY_A, KEY_D, KEY_S, engine, step};

const SHIFT: u8 = 0xE1;

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
