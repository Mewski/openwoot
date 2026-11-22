use openwoot::{
    hid::InputReport,
    keys::{RapidTrigger, RapidTriggerOverride, Tuning},
    usb::Mode,
};
use openwoot_protocol::keys::Action;

use super::support::{KEY_A, KEY_D, KEY_F, KEY_S, TUNING, engine, step};

const SHIFT: u8 = 0xE1;

#[test]
fn typing_reports_the_usage_past_the_actuation_point_and_releases_with_hysteresis() {
    let mut keys = engine(&[
        (KEY_A, Action::Keyboard(0x04)),
        (KEY_S, Action::Keyboard(SHIFT)),
    ]);
    assert_eq!(step(&mut keys, &[(KEY_A, 999)], 1), []);
    assert_eq!(step(&mut keys, &[(KEY_A, 1000)], 1), [0x04]);
    assert_eq!(
        step(&mut keys, &[(KEY_A, 905), (KEY_S, 3000)], 1),
        [0x04, SHIFT]
    );
    assert_eq!(step(&mut keys, &[(KEY_A, 903), (KEY_S, 3000)], 1), [SHIFT]);
    assert_eq!(step(&mut keys, &[(KEY_A, 0), (KEY_S, 0)], 1), []);
}

#[test]
fn rapid_trigger_releases_after_an_upstroke_and_presses_again_on_the_way_down() {
    let mut keys = engine(&[(KEY_A, Action::Keyboard(0x04))]);
    keys.set_tuning(Tuning {
        rapid_trigger_enabled: true,
        ..TUNING
    });
    let pressed: Vec<bool> = [1000, 2500, 2301, 2299, 2400, 2599, 2601, 905, 903, 1000]
        .into_iter()
        .map(|travel| !step(&mut keys, &[(KEY_A, travel)], 1).is_empty())
        .collect();
    assert_eq!(
        pressed,
        [
            true, true, true, false, false, false, true, false, false, true
        ]
    );
}

#[test]
fn continuous_rapid_trigger_keeps_working_until_the_key_is_at_rest() {
    let mut keys = engine(&[(KEY_A, Action::Keyboard(0x04))]);
    keys.set_tuning(Tuning {
        rapid_trigger_enabled: true,
        rapid_trigger: RapidTrigger {
            continuous: true,
            ..TUNING.rapid_trigger
        },
        ..TUNING
    });
    let pressed: Vec<bool> = [1000, 700, 300, 501, 400]
        .into_iter()
        .map(|travel| !step(&mut keys, &[(KEY_A, travel)], 1).is_empty())
        .collect();
    assert_eq!(pressed, [true, false, false, true, true]);
    assert_eq!(step(&mut keys, &[(KEY_A, 0)], 1), []);
}

#[test]
fn a_key_override_gives_one_key_its_own_actuation_and_rapid_trigger() {
    let mut keys = engine(&[
        (KEY_A, Action::Keyboard(0x04)),
        (KEY_D, Action::Keyboard(0x07)),
    ]);
    keys.set_override(
        KEY_D,
        Some(400),
        RapidTriggerOverride::With(RapidTrigger {
            press: 100,
            release: 100,
            continuous: false,
        }),
    );
    assert_eq!(step(&mut keys, &[(KEY_A, 500), (KEY_D, 500)], 1), [0x07]);
    assert_eq!(step(&mut keys, &[(KEY_A, 500), (KEY_D, 399)], 1), []);
    assert_eq!(step(&mut keys, &[(KEY_A, 500), (KEY_D, 600)], 1), []);
    assert_eq!(step(&mut keys, &[(KEY_A, 500), (KEY_D, 601)], 1), [0x07]);
}

#[test]
fn the_nkro_report_places_usages_where_the_descriptor_declares_them() {
    let mut keys = engine(&[
        (KEY_A, Action::Keyboard(0x9D)),
        (KEY_S, Action::Keyboard(0xDD)),
    ]);
    step(&mut keys, &[(KEY_A, 2000), (KEY_S, 2000)], 1);
    let openwoot::keys::Keyboard::NKeyRollover(report) = keys.output().keyboard() else {
        panic!("boot format");
    };
    let bytes = report.to_bytes();
    assert_eq!(bytes.get(21), Some(&0x01));
    assert_eq!(bytes.get(27), Some(&0x20));
    assert_eq!(bytes.iter().map(|byte| byte.count_ones()).sum::<u32>(), 2);
}

#[test]
fn the_boot_report_takes_six_keys_and_drops_the_seventh() {
    let keys_and_usages: Vec<_> = (0..7u8)
        .map(|column| {
            (
                openwoot::keys::Key::new(2, column),
                Action::Keyboard(0x04 + column),
            )
        })
        .collect();
    let mut keys = engine(&keys_and_usages);
    keys.set_boot_format(true);
    let events: Vec<_> = keys_and_usages
        .iter()
        .map(|(key, _)| (*key, 2000))
        .collect();
    assert_eq!(step(&mut keys, &events, 1), [4, 5, 6, 7, 8, 9]);
}

#[test]
fn keyboard_keys_are_muted_in_the_gamepad_modes_unless_the_profile_allows_them() {
    let mut keys = engine(&[(KEY_F, Action::Keyboard(0x09))]);
    keys.set_mode(Mode::Gamepad);
    assert_eq!(step(&mut keys, &[(KEY_F, 2000)], 1), []);
    keys.set_tuning(Tuning {
        keyboard_in_gamepad_mode: true,
        ..TUNING
    });
    assert_eq!(step(&mut keys, &[(KEY_F, 2000)], 1), [0x09]);
}

#[test]
fn with_key_reports_off_presses_reach_no_report_until_they_are_on_again() {
    let mut keys = engine(&[(KEY_A, Action::Keyboard(0x04))]);
    keys.set_key_reports(false);
    assert_eq!(step(&mut keys, &[(KEY_A, 3000)], 1), []);
    keys.set_key_reports(true);
    assert_eq!(step(&mut keys, &[(KEY_A, 3001)], 1), [0x04]);
}
