use openwoot::keys::{Behaviour, Entry, Key};
use openwoot_protocol::{
    advanced::{MAX_ENTRIES, Resolution},
    keys::{Action, Layer},
};

use super::support::{KEY_A, SWITCH, engine, step};

const OUTSIDE: Key = Key::new(6, 20);

#[test]
fn keys_layers_and_entries_beyond_the_tables_are_ignored() {
    let mut keys = engine(&[(KEY_A, Action::Keyboard(0x04))]);
    keys.set_switch(OUTSIDE, SWITCH);
    keys.set_action(OUTSIDE, Layer::Three, Action::Keyboard(0x05));
    keys.set_gamepad_binding(OUTSIDE, None);
    keys.set_advanced((0..48u8).map(|number| Entry {
        key: if number % 2 == 0 { OUTSIDE } else { KEY_A },
        layer: number % 7,
        behaviour: Behaviour::Socd {
            other: OUTSIDE,
            resolution: Resolution::Both,
            both_when_bottomed_out: true,
        },
    }));
    assert_eq!(keys.advanced().entries().len(), MAX_ENTRIES);
    let storm: Vec<(Key, u16)> = (0..128u16)
        .map(|number| {
            let key = if number % 3 == 0 { OUTSIDE } else { KEY_A };
            (key, number.wrapping_mul(521))
        })
        .collect();
    step(&mut keys, &storm, u16::MAX);
    step(&mut keys, &[(OUTSIDE, u16::MAX), (KEY_A, u16::MAX)], 0);
    assert_eq!(step(&mut keys, &[(KEY_A, u16::MAX)], 1), [0x04]);
    assert_eq!(step(&mut keys, &[(KEY_A, 0)], 1), []);
    assert_eq!(keys.actuation(OUTSIDE), None);
}
