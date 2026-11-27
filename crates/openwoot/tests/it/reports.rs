use openwoot::hid::{
    BOOT_KEYBOARD_DESCRIPTOR, BootReport, CONTROLS_DESCRIPTOR, ConsumerReport, GAMEPAD_DESCRIPTOR,
    GAMEPAD_SLIDER_MAX, GAMEPAD_STICK_CENTER, GAMEPAD_STICK_MAX, GamepadReport, Hat, InputReport,
    KEY_STATE_ENTRIES, KeyStateEntry as Entry, KeyStateKind as Kind, KeyStateReport, Leds,
    Modifiers, MouseReport, NKRO_DESCRIPTOR, NkroReport, SystemControl, SystemReport, Travel,
};
use openwoot_protocol::keys::Action;

use crate::support::{Kind as Direction, parse_report_descriptor as parse};

const DESKTOP_PAGE: u32 = 0x0001_0000;
const SIMULATION_PAGE: u32 = 0x0002_0000;
const KEYBOARD_PAGE: u32 = 0x0007_0000;
const LED_PAGE: u32 = 0x0008_0000;
const BUTTON_PAGE: u32 = 0x0009_0000;
const CONSUMER_PAGE: u32 = 0x000C_0000;
const ERROR_ROLL_OVER: u32 = KEYBOARD_PAGE | 0x01;

#[test]
fn boot_report_has_modifiers_a_reserved_byte_and_six_keys() {
    let mut report = BootReport::default();
    assert!(report.press(0xE1));
    for usage in 0x04..0x0A {
        assert!(report.press(usage));
    }
    assert!(!report.is_rolled_over());
    assert_eq!(
        report.to_bytes(),
        [0x02, 0, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09]
    );
    report.release(0x05);
    report.release(0xE1);
    assert_eq!(report.to_bytes(), [0, 0, 0x04, 0, 0x06, 0x07, 0x08, 0x09]);
}

#[test]
fn a_seventh_key_reads_as_error_roll_over_in_every_slot_and_keeps_the_modifiers() {
    let parsed = parse(BOOT_KEYBOARD_DESCRIPTOR).unwrap();
    let mut report = BootReport::default();
    assert!(report.press(0xE0));
    for usage in 0x04..0x0A {
        assert!(report.press(usage));
    }
    assert!(!report.press(0x0A));
    assert!(report.is_rolled_over());
    assert_eq!(report.to_bytes(), [0x01, 0, 1, 1, 1, 1, 1, 1]);
    let mut expected = vec![KEYBOARD_PAGE | 0xE0];
    expected.extend([ERROR_ROLL_OVER; 6]);
    assert_eq!(
        parsed.pressed(Direction::Input, 0, &report.to_bytes()),
        expected
    );
    report.release(0x04);
    assert!(!report.is_rolled_over());
    assert!(report.press(0x0A));
    assert_eq!(
        report.to_bytes(),
        [0x01, 0, 0x0A, 0x05, 0x06, 0x07, 0x08, 0x09]
    );
}

#[test]
fn the_boot_descriptor_reads_the_modifiers_and_keys_of_a_boot_report() {
    let parsed = parse(BOOT_KEYBOARD_DESCRIPTOR).unwrap();
    let mut report = BootReport::default();
    for usage in [0xE1, 0xE6, 0x04, 0xDD] {
        assert!(report.press(usage));
    }
    let bytes = report.to_bytes();
    assert_eq!(
        parsed.pressed(Direction::Input, 0, &bytes),
        [
            KEYBOARD_PAGE | 0xE1,
            KEYBOARD_PAGE | 0xE6,
            KEYBOARD_PAGE | 0x04,
            KEYBOARD_PAGE | 0xDD
        ]
    );
    assert_eq!(
        parsed.value(Direction::Input, 0, KEYBOARD_PAGE | 0x05, &bytes),
        Some(0)
    );
    assert_eq!(
        parsed.value(Direction::Input, 0, KEYBOARD_PAGE | 0xE6, &bytes),
        Some(1)
    );
}

#[test]
fn the_keyboard_descriptors_place_the_lock_lights_where_leds_reads_them() {
    for descriptor in [BOOT_KEYBOARD_DESCRIPTOR, NKRO_DESCRIPTOR] {
        let parsed = parse(descriptor).unwrap();
        let lit = |bits: u8| parsed.pressed(Direction::Output, 0, &[bits]);
        assert!(Leds::from_bits(0x01).num_lock());
        assert_eq!(lit(0x01), [LED_PAGE | 1]);
        assert!(Leds::from_bits(0x02).caps_lock());
        assert_eq!(lit(0x02), [LED_PAGE | 2]);
        assert!(Leds::from_bits(0x04).scroll_lock());
        assert_eq!(lit(0x04), [LED_PAGE | 3]);
        assert!(Leds::from_bits(0x08).compose());
        assert_eq!(lit(0x08), [LED_PAGE | 4]);
        assert!(Leds::from_bits(0x10).kana());
        assert_eq!(lit(0x10), [LED_PAGE | 5]);
        assert_eq!(lit(0xE0), []);
    }
}

#[test]
fn nkro_report_is_a_modifier_byte_and_a_27_byte_bitmap() {
    let mut report = NkroReport::new();
    assert!(report.set(0xE7, true));
    assert!(report.set(0x04, true));
    assert!(report.set(0x33, true));
    assert!(report.set(0xDD, true));
    assert!(!report.set(0x32, true));
    let bytes = report.to_bytes();
    assert_eq!(bytes.len(), 28);
    assert_eq!(bytes[0], 0x80);
    assert_eq!(bytes[1], 0x01);
    assert_eq!(bytes[7], 0x01);
    assert_eq!(bytes[27], 0x20);
    assert_eq!(Modifiers::from_bits(0x80).bits(), 0x80);
}

#[test]
fn leds_decode_the_lock_bits() {
    let leds = Leds::from_bits(0xFF);
    assert_eq!(leds.bits(), 0x1F);
    assert!(leds.num_lock() && leds.caps_lock() && leds.scroll_lock());
    assert!(leds.compose() && leds.kana());
    assert!(!Leds::from_bits(0x01).caps_lock());
}

#[test]
fn controls_reports_start_with_their_report_id() {
    assert_eq!(
        SystemReport {
            control: Some(SystemControl::Sleep)
        }
        .to_bytes(),
        [1, 2]
    );
    assert_eq!(SystemReport::default().to_bytes(), [1, 0]);
    assert_eq!(SystemControl::WakeUp.usage(), 0x83);
    assert_eq!(
        ConsumerReport {
            usages: [0x00E9, 0x02A2, 0]
        }
        .to_bytes(),
        [2, 0xE9, 0x00, 0xA2, 0x02, 0, 0]
    );
    assert_eq!(
        MouseReport {
            buttons: 0xFF,
            x: -1,
            y: 5,
            wheel: i8::MIN,
            pan: 127,
        }
        .to_bytes(),
        [3, 0x1F, 0xFF, 0x05, 0x81, 0x7F]
    );
}

#[test]
fn the_controls_descriptor_reads_each_report_behind_its_report_id() {
    let parsed = parse(CONTROLS_DESCRIPTOR).unwrap();
    let system = SystemReport {
        control: Some(SystemControl::Sleep),
    }
    .to_bytes();
    assert_eq!(system[0], 1);
    assert_eq!(
        parsed.pressed(Direction::Input, 1, &system[1..]),
        [DESKTOP_PAGE | u32::from(SystemControl::Sleep.usage())]
    );
    let idle = SystemReport::default().to_bytes();
    assert_eq!(parsed.pressed(Direction::Input, 1, &idle[1..]), []);
    let consumer = ConsumerReport {
        usages: [0x00E9, 0x02A2, 0],
    }
    .to_bytes();
    assert_eq!(consumer[0], 2);
    assert_eq!(
        parsed.pressed(Direction::Input, 2, &consumer[1..]),
        [CONSUMER_PAGE | 0x00E9, CONSUMER_PAGE | 0x02A2]
    );
    let mouse = MouseReport {
        buttons: 0x05,
        x: i8::MIN,
        y: 7,
        wheel: -3,
        pan: 2,
    }
    .to_bytes();
    assert_eq!(mouse[0], 3);
    let read = |usage| parsed.value(Direction::Input, 3, usage, &mouse[1..]);
    assert_eq!(read(BUTTON_PAGE | 1), Some(1));
    assert_eq!(read(BUTTON_PAGE | 2), Some(0));
    assert_eq!(read(BUTTON_PAGE | 3), Some(1));
    assert_eq!(read(DESKTOP_PAGE | 0x30), Some(-127));
    assert_eq!(read(DESKTOP_PAGE | 0x31), Some(7));
    assert_eq!(read(DESKTOP_PAGE | 0x38), Some(-3));
    assert_eq!(read(CONSUMER_PAGE | 0x0238), Some(2));
    for axis in [
        DESKTOP_PAGE | 0x30,
        DESKTOP_PAGE | 0x38,
        CONSUMER_PAGE | 0x0238,
    ] {
        assert_eq!(
            parsed.logical_range(Direction::Input, 3, axis),
            Some((-127, 127))
        );
    }
}

#[test]
fn gamepad_report_has_buttons_six_axes_five_sliders_and_a_hat() {
    let report = GamepadReport {
        buttons: 0xFFFF,
        x: 0x0102,
        y: 0x0304,
        z: 0x0506,
        rz: 0x0708,
        rx: 0x090A,
        ry: 0x0B0C,
        throttle: 0x0D0E,
        elevator: 0x0F10,
        accelerator: 0x1112,
        rudder: 0x1314,
        trigger: 0x1516,
        hat: Hat::UpLeft,
    };
    assert_eq!(
        report.to_bytes(),
        [
            0xFF, 0x0F, 0x02, 0x01, 0x04, 0x03, 0x06, 0x05, 0x08, 0x07, 0x0A, 0x09, 0x0C, 0x0B,
            0x0E, 0x0D, 0x10, 0x0F, 0x12, 0x11, 0x14, 0x13, 0x16, 0x15, 8
        ]
    );
    let idle = GamepadReport::new().to_bytes();
    assert_eq!(idle[2..4], GAMEPAD_STICK_CENTER.to_le_bytes());
    assert_eq!(idle[24], Hat::Centered.value());
}

#[test]
fn the_gamepad_descriptor_reads_every_control_of_a_gamepad_report() {
    let parsed = parse(GAMEPAD_DESCRIPTOR).unwrap();
    let report = GamepadReport {
        buttons: 0x0805,
        x: 0x0102,
        y: 0x0304,
        z: 0x0506,
        rz: 0x0708,
        rx: 0x090A,
        ry: 0x0B0C,
        throttle: 0x0D0E,
        elevator: 0x0F10,
        accelerator: 0x1112,
        rudder: 0x1314,
        trigger: 0x1516,
        hat: Hat::Left,
    };
    let bytes = report.to_bytes();
    let read = |usage| parsed.value(Direction::Input, 0, usage, &bytes);
    assert_eq!(read(BUTTON_PAGE | 1), Some(1));
    assert_eq!(read(BUTTON_PAGE | 2), Some(0));
    assert_eq!(read(BUTTON_PAGE | 3), Some(1));
    assert_eq!(read(BUTTON_PAGE | 0x0C), Some(1));
    for (usage, value) in [
        (DESKTOP_PAGE | 0x30, report.x),
        (DESKTOP_PAGE | 0x31, report.y),
        (DESKTOP_PAGE | 0x32, report.z),
        (DESKTOP_PAGE | 0x35, report.rz),
        (DESKTOP_PAGE | 0x33, report.rx),
        (DESKTOP_PAGE | 0x34, report.ry),
        (SIMULATION_PAGE | 0xBB, report.throttle),
        (SIMULATION_PAGE | 0xB8, report.elevator),
        (SIMULATION_PAGE | 0xC4, report.accelerator),
        (SIMULATION_PAGE | 0xBA, report.rudder),
        (SIMULATION_PAGE | 0xC0, report.trigger),
        (DESKTOP_PAGE | 0x39, u16::from(report.hat.value())),
    ] {
        assert_eq!(read(usage), Some(i64::from(value)), "usage {usage:#010x}");
    }
    assert_eq!(
        parsed.logical_range(Direction::Input, 0, DESKTOP_PAGE | 0x30),
        Some((0, i64::from(GAMEPAD_STICK_MAX)))
    );
    assert_eq!(
        parsed.logical_range(Direction::Input, 0, SIMULATION_PAGE | 0xBB),
        Some((0, i64::from(GAMEPAD_SLIDER_MAX)))
    );
    assert_eq!(
        parsed.logical_range(Direction::Input, 0, DESKTOP_PAGE | 0x39),
        Some((1, 8))
    );
    let centered = GamepadReport::new().to_bytes();
    assert_eq!(
        parsed.value(Direction::Input, 0, DESKTOP_PAGE | 0x39, &centered),
        Some(i64::from(Hat::Centered.value()))
    );
}

#[test]
fn key_state_report_packs_sixteen_entries_of_four_bytes() {
    let mut report = KeyStateReport::default();
    let entry = Entry {
        key: 0x21,
        code: 0x04,
        pressed: true,
        kind: Kind::Advanced,
        travel: Travel::MAX,
    };
    for _ in 0..KEY_STATE_ENTRIES {
        assert!(report.push(entry));
    }
    assert!(!report.push(entry));
    let bytes = report.to_bytes();
    assert_eq!(bytes.len(), 64);
    assert_eq!(bytes[..4], [0x21, 0x04, 0xD9, 0xFF]);
    assert_eq!(bytes[60..], [0x21, 0x04, 0xD9, 0xFF]);
    report.clear();
    assert!(report.push(Entry {
        travel: Travel::new(0x155).unwrap(),
        kind: Kind::Gamepad,
        ..Entry::default()
    }));
    assert_eq!(report.to_bytes()[..8], [0, 0, 0x54, 0x55, 0, 0, 0, 0]);
    assert_eq!(Travel::new(0x400), None);
}

#[test]
fn every_keyboard_usage_a_key_may_carry_sets_its_own_bit_in_the_nkro_report() {
    let parsed = parse(NKRO_DESCRIPTOR).unwrap();
    for usage in Action::KEYBOARD_USAGES.into_iter().flatten() {
        let mut report = NkroReport::new();
        assert!(report.set(usage, true), "usage {usage:#04x}");
        assert!(report.is_pressed(usage), "usage {usage:#04x}");
        let bytes = report.to_bytes();
        assert_eq!(
            bytes.iter().map(|byte| byte.count_ones()).sum::<u32>(),
            1,
            "usage {usage:#04x}"
        );
        assert_eq!(
            parsed.pressed(Direction::Input, 0, &bytes),
            [KEYBOARD_PAGE | u32::from(usage)],
            "usage {usage:#04x}"
        );
    }
}

#[test]
fn gamepad_axes_above_their_logical_maximum_are_sent_at_the_maximum() {
    let parsed = parse(GAMEPAD_DESCRIPTOR).unwrap();
    let report = GamepadReport {
        buttons: 0,
        x: u16::MAX,
        y: GAMEPAD_STICK_MAX,
        z: GAMEPAD_STICK_MAX + 1,
        rz: 0,
        rx: u16::MAX,
        ry: u16::MAX,
        throttle: u16::MAX,
        elevator: GAMEPAD_SLIDER_MAX,
        accelerator: u16::MAX,
        rudder: u16::MAX,
        trigger: u16::MAX,
        hat: Hat::Centered,
    };
    let bytes = report.to_bytes();
    let read = |usage| parsed.value(Direction::Input, 0, usage, &bytes);
    for (usage, maximum) in [
        (DESKTOP_PAGE | 0x30, GAMEPAD_STICK_MAX),
        (DESKTOP_PAGE | 0x31, GAMEPAD_STICK_MAX),
        (DESKTOP_PAGE | 0x32, GAMEPAD_STICK_MAX),
        (DESKTOP_PAGE | 0x33, GAMEPAD_STICK_MAX),
        (DESKTOP_PAGE | 0x34, GAMEPAD_STICK_MAX),
        (SIMULATION_PAGE | 0xBB, GAMEPAD_SLIDER_MAX),
        (SIMULATION_PAGE | 0xB8, GAMEPAD_SLIDER_MAX),
        (SIMULATION_PAGE | 0xC4, GAMEPAD_SLIDER_MAX),
        (SIMULATION_PAGE | 0xBA, GAMEPAD_SLIDER_MAX),
        (SIMULATION_PAGE | 0xC0, GAMEPAD_SLIDER_MAX),
    ] {
        assert_eq!(read(usage), Some(i64::from(maximum)), "usage {usage:#010x}");
    }
    assert_eq!(read(DESKTOP_PAGE | 0x35), Some(0));
}

#[test]
fn each_report_leads_with_the_report_id_its_descriptor_declares() {
    let controls = parse(CONTROLS_DESCRIPTOR).unwrap();
    let payload_bits =
        |report_id: Option<u8>| report_id.map(|id| controls.report_bits(Direction::Input, id));
    assert_eq!(SystemReport::ID, Some(1));
    assert_eq!(payload_bits(SystemReport::ID), Some(8 * SystemReport::LEN));
    assert_eq!(
        SystemReport::default().to_bytes().first().copied(),
        SystemReport::ID
    );
    assert_eq!(
        SystemReport::default().to_bytes().len(),
        1 + SystemReport::LEN
    );
    assert_eq!(ConsumerReport::ID, Some(2));
    assert_eq!(
        payload_bits(ConsumerReport::ID),
        Some(8 * ConsumerReport::LEN)
    );
    assert_eq!(
        ConsumerReport::default().to_bytes().first().copied(),
        ConsumerReport::ID
    );
    assert_eq!(
        ConsumerReport::default().to_bytes().len(),
        1 + ConsumerReport::LEN
    );
    assert_eq!(MouseReport::ID, Some(3));
    assert_eq!(payload_bits(MouseReport::ID), Some(8 * MouseReport::LEN));
    assert_eq!(
        MouseReport::default().to_bytes().first().copied(),
        MouseReport::ID
    );
    assert_eq!(
        MouseReport::default().to_bytes().len(),
        1 + MouseReport::LEN
    );
    assert_eq!(BootReport::ID, None);
    assert_eq!(NkroReport::ID, None);
    assert_eq!(GamepadReport::ID, None);
    assert_eq!(KeyStateReport::ID, None);
}
