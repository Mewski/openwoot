use openwoot::hid::{
    BootReport, ConsumerReport, GAMEPAD_STICK_CENTER, GamepadReport, Hat, InputReport,
    KEY_STATE_ENTRIES, KeyStateEntry as Entry, KeyStateKind as Kind, KeyStateReport, Leds,
    Modifiers, MouseReport, NkroReport, SystemControl, SystemReport, Travel,
};

#[test]
fn boot_report_has_modifiers_a_reserved_byte_and_six_keys() {
    let mut report = BootReport::default();
    assert!(report.press(0xE1));
    for usage in 0x04..0x0A {
        assert!(report.press(usage));
    }
    assert!(!report.press(0x0A));
    assert_eq!(
        report.to_bytes(),
        [0x02, 0, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09]
    );
    report.release(0x05);
    report.release(0xE1);
    assert_eq!(report.to_bytes(), [0, 0, 0x04, 0, 0x06, 0x07, 0x08, 0x09]);
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
            y: 5
        }
        .to_bytes(),
        [3, 0x1F, 0xFF, 0x05]
    );
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
