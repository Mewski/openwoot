use openwoot::keys::{Key, Notice};
use openwoot_protocol::{
    keys::{Action, Function, Layer},
    profile::Slot,
};

use super::support::{KEY_A, KEY_S, engine, step};

const FN: Key = Key::new(5, 12);
const VOLUME_UP: u16 = 0xE9;

#[test]
fn the_fn_layer_applies_while_held_and_a_key_keeps_its_layer_until_released() {
    let mut keys = engine(&[
        (FN, Action::Layer(Layer::One)),
        (KEY_A, Action::Keyboard(0x04)),
    ]);
    keys.set_action(KEY_A, Layer::One, Action::Consumer(VOLUME_UP));
    step(&mut keys, &[(FN, 2000)], 1);
    assert_eq!(keys.layers().active(), 1);
    step(&mut keys, &[(FN, 2000), (KEY_A, 2000)], 1);
    assert_eq!(keys.output().consumer_usages(), [VOLUME_UP]);
    step(&mut keys, &[(FN, 0), (KEY_A, 2000)], 1);
    assert_eq!(keys.layers().active(), 0);
    assert_eq!(keys.output().consumer_usages(), [VOLUME_UP]);
    assert_eq!(step(&mut keys, &[(KEY_A, 0)], 1), []);
    assert_eq!(step(&mut keys, &[(KEY_A, 2000)], 1), [0x04]);
}

#[test]
fn layer_lock_locks_the_layer_of_its_key_and_a_second_press_unlocks() {
    let mut keys = engine(&[(FN, Action::Layer(Layer::One)), (KEY_S, Action::None)]);
    keys.set_action(KEY_S, Layer::One, Action::Function(Function::LayerLock));
    step(&mut keys, &[(FN, 2000)], 1);
    step(&mut keys, &[(FN, 2000), (KEY_S, 2000)], 1);
    step(&mut keys, &[(FN, 0), (KEY_S, 0)], 1);
    assert_eq!((keys.layers().active(), keys.layers().locked()), (1, 1));
    step(&mut keys, &[(KEY_S, 2000)], 1);
    step(&mut keys, &[(KEY_S, 0)], 1);
    assert_eq!((keys.layers().active(), keys.layers().locked()), (0, 0));
}

#[test]
fn a_profile_key_requests_the_profile_and_stays_silent_until_released() {
    let mut keys = engine(&[(KEY_A, Action::Function(Function::Profile(Slot::new(2))))]);
    keys.set_profiles(4, 0);
    step(&mut keys, &[(KEY_A, 2000)], 1);
    assert_eq!(keys.take_profile_request(), Some(2));
    assert!(keys.holds_events(KEY_A));
    assert!(keys.take_notices().contains(&Notice::ProfileBlink(KEY_A)));
    step(&mut keys, &[(KEY_A, 0)], 1);
    assert!(!keys.holds_events(KEY_A));
    keys.set_profiles(4, 2);
    step(&mut keys, &[(KEY_A, 2000)], 1);
    assert_eq!(keys.take_profile_request(), None);
}

#[test]
fn next_profile_wraps_after_the_last_profile() {
    let mut keys = engine(&[(KEY_A, Action::Function(Function::NextProfile))]);
    keys.set_profiles(4, 3);
    step(&mut keys, &[(KEY_A, 2000)], 1);
    assert_eq!(keys.take_profile_request(), Some(0));
}

#[test]
fn the_key_disable_toggle_blocks_its_key_and_only_lets_itself_through() {
    let mut keys = engine(&[
        (KEY_A, Action::Keyboard(0x04)),
        (FN, Action::Layer(Layer::One)),
    ]);
    keys.set_action(
        KEY_A,
        Layer::One,
        Action::Function(Function::ToggleKeyDisable),
    );
    step(&mut keys, &[(FN, 2000)], 1);
    step(&mut keys, &[(FN, 2000), (KEY_A, 2000)], 1);
    assert!(keys.take_notices().contains(&Notice::Disabled {
        key: KEY_A,
        disabled: true
    }));
    step(&mut keys, &[(FN, 0), (KEY_A, 0)], 1);
    assert_eq!(step(&mut keys, &[(KEY_A, 2000)], 1), []);
}

#[test]
fn brightness_keys_start_and_stop_the_lighting_repeat() {
    let mut keys = engine(&[(KEY_A, Action::Function(Function::BrightnessUp))]);
    step(&mut keys, &[(KEY_A, 2000)], 1);
    step(&mut keys, &[(KEY_A, 0)], 1);
    let notices = keys.take_notices();
    assert!(notices.contains(&Notice::Brightness {
        increase: true,
        held: true
    }));
    assert!(notices.contains(&Notice::Brightness {
        increase: true,
        held: false
    }));
}
