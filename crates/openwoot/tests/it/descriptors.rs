use openwoot::{
    hid::{
        BOOT_KEYBOARD_DESCRIPTOR, BootReport, CHANNEL_DESCRIPTOR, CHANNEL_FEATURE_REPORT_LEN,
        CONTROLS_DESCRIPTOR, ConsumerReport, GAMEPAD_DESCRIPTOR, GamepadReport, InputReport,
        KEY_STATE_DESCRIPTOR, KeyStateReport, MouseReport, NKRO_DESCRIPTOR, NkroReport,
        PLACEHOLDER_DESCRIPTOR, PLACEHOLDER_LEN, SystemReport, XinputReport,
    },
    usb::{Face, KeyboardFormat, Mode, PollingRate, Speed},
};
use openwoot_protocol::REPORT_SIZES;

use crate::support::{
    IDENTITY, Kind, Usages, configuration, enumerate, faces, parse_configuration,
    parse_report_descriptor as parse,
};

const XINPUT_CLASS_DESCRIPTOR: [u8; 17] = [
    0x11, 0x21, 0x00, 0x01, 0x01, 0x25, 0x81, 0x14, 0x00, 0x00, 0x00, 0x00, 0x13, 0x01, 0x08, 0x00,
    0x00,
];

fn bytes_of(kind: Kind, report_id: u8, descriptor: &[u8]) -> usize {
    parse(descriptor).map_or(0, |parsed| parsed.report_bits(kind, report_id) / 8)
}

#[test]
fn report_descriptors_parse_into_the_sizes_of_the_typed_reports() {
    assert_eq!(
        bytes_of(Kind::Input, 0, BOOT_KEYBOARD_DESCRIPTOR),
        BootReport::LEN
    );
    assert_eq!(bytes_of(Kind::Output, 0, BOOT_KEYBOARD_DESCRIPTOR), 1);
    assert_eq!(bytes_of(Kind::Input, 0, NKRO_DESCRIPTOR), NkroReport::LEN);
    assert_eq!(bytes_of(Kind::Output, 0, NKRO_DESCRIPTOR), 1);
    assert_eq!(
        bytes_of(Kind::Input, 1, CONTROLS_DESCRIPTOR),
        SystemReport::LEN
    );
    assert_eq!(
        bytes_of(Kind::Input, 2, CONTROLS_DESCRIPTOR),
        ConsumerReport::LEN
    );
    assert_eq!(
        bytes_of(Kind::Input, 3, CONTROLS_DESCRIPTOR),
        MouseReport::LEN
    );
    assert_eq!(
        bytes_of(Kind::Input, 0, GAMEPAD_DESCRIPTOR),
        GamepadReport::LEN
    );
    assert_eq!(
        bytes_of(Kind::Input, 0, KEY_STATE_DESCRIPTOR),
        KeyStateReport::LEN
    );
    assert_eq!(
        bytes_of(Kind::Input, 0, PLACEHOLDER_DESCRIPTOR),
        PLACEHOLDER_LEN
    );
    for (report_id, size) in (1..).zip(REPORT_SIZES) {
        assert_eq!(bytes_of(Kind::Input, report_id, CHANNEL_DESCRIPTOR), size);
        assert_eq!(bytes_of(Kind::Output, report_id, CHANNEL_DESCRIPTOR), size);
    }
    assert_eq!(
        bytes_of(Kind::Feature, 1, CHANNEL_DESCRIPTOR),
        CHANNEL_FEATURE_REPORT_LEN
    );
    assert_eq!(XinputReport::LEN, 20);
}

#[test]
fn every_report_fills_whole_bytes() {
    for descriptor in [
        BOOT_KEYBOARD_DESCRIPTOR,
        NKRO_DESCRIPTOR,
        CONTROLS_DESCRIPTOR,
        GAMEPAD_DESCRIPTOR,
        KEY_STATE_DESCRIPTOR,
        PLACEHOLDER_DESCRIPTOR,
        CHANNEL_DESCRIPTOR,
    ] {
        let parsed = parse(descriptor).unwrap();
        for (report, bits) in parsed.reports() {
            assert_eq!(bits % 8, 0, "{report:?}");
        }
    }
}

#[test]
fn applications_keep_the_stock_usages() {
    let application = |descriptor| parse(descriptor).unwrap().applications();
    assert_eq!(application(BOOT_KEYBOARD_DESCRIPTOR), [0x0001_0006]);
    assert_eq!(application(NKRO_DESCRIPTOR), [0x0001_0006]);
    assert_eq!(
        application(CONTROLS_DESCRIPTOR),
        [0x0001_0080, 0x000C_0001, 0x0001_0002]
    );
    assert_eq!(application(GAMEPAD_DESCRIPTOR), [0x0001_0005]);
    assert_eq!(application(KEY_STATE_DESCRIPTOR), [0xFF53_0001]);
    assert_eq!(application(PLACEHOLDER_DESCRIPTOR), [0xFF00_0001]);
    assert_eq!(application(CHANNEL_DESCRIPTOR), [0xFF55_0001]);
}

#[test]
fn nkro_bitmap_follows_the_descriptor() {
    let parsed = parse(NKRO_DESCRIPTOR).unwrap();
    for usage in 0..=u8::MAX {
        let full_usage = 0x0007_0000 | u32::from(usage);
        let bit = parsed.fields.iter().find_map(|field| match field.usages {
            Usages::Range { minimum, maximum }
                if field.kind == Kind::Input && (minimum..=maximum).contains(&full_usage) =>
            {
                Some(field.bit_offset + usize::try_from(full_usage - minimum).unwrap())
            }
            _ => None,
        });
        let mut report = NkroReport::new();
        assert_eq!(report.set(usage, true), bit.is_some(), "usage {usage:#04x}");
        let bytes = report.to_bytes();
        let set_bits: Vec<usize> = (0..bytes.len() * 8)
            .filter(|bit| bytes[bit / 8] & (1 << (bit % 8)) != 0)
            .collect();
        assert_eq!(
            set_bits,
            bit.into_iter().collect::<Vec<_>>(),
            "usage {usage:#04x}"
        );
        assert_eq!(report.is_pressed(usage), bit.is_some());
        report.set(usage, false);
        assert_eq!(report, NkroReport::new());
    }
}

#[test]
fn every_face_enumerates_with_the_stock_interfaces() {
    for face in faces() {
        let configuration = configuration(face).unwrap();
        assert_eq!(configuration.interface_count, 5, "{face:?}");
        assert_eq!(configuration.value, 1);
        assert_eq!(configuration.attributes, 0xA0);
        assert_eq!(configuration.max_power, 200);
        let interfaces = &configuration.interfaces;
        assert_eq!(
            interfaces
                .iter()
                .map(|interface| (interface.number, interface.alternate))
                .collect::<Vec<_>>(),
            [(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]
        );
        let gamepad_interval = match (face.speed, face.gamepad_polling) {
            (Speed::High, PollingRate::Every4Milliseconds) => 6,
            (Speed::High, PollingRate::Every1Millisecond)
            | (Speed::Full, PollingRate::Every4Milliseconds) => 4,
            _ => 1,
        };
        let endpoints = |number: usize| {
            interfaces[number]
                .endpoints
                .iter()
                .map(|endpoint| {
                    assert_eq!(endpoint.attributes, 0x03);
                    (
                        endpoint.address,
                        endpoint.max_packet_size,
                        endpoint.interval,
                    )
                })
                .collect::<Vec<_>>()
        };
        let classes = |number: usize| {
            let interface = &interfaces[number];
            (interface.class, interface.subclass, interface.protocol)
        };
        match face.mode {
            Mode::Xinput => {
                assert_eq!(classes(0), (0xFF, 0x5D, 0x01));
                assert_eq!(
                    interfaces[0].class_descriptors,
                    [XINPUT_CLASS_DESCRIPTOR.to_vec()]
                );
                assert_eq!(
                    endpoints(0),
                    [(0x81, 32, gamepad_interval), (0x01, 32, gamepad_interval)]
                );
            }
            Mode::Gamepad => {
                assert_eq!(classes(0), (0x03, 0, 0));
                assert_eq!(endpoints(0), [(0x81, 32, gamepad_interval)]);
            }
            Mode::NoGamepad => {
                assert_eq!(classes(0), (0x03, 0, 0));
                assert_eq!(endpoints(0), [(0x81, 8, 10)]);
            }
        }
        assert_eq!(classes(1), (0x03, 0x01, 0x01));
        match face.keyboard {
            KeyboardFormat::NKeyRollover => assert_eq!(endpoints(1), [(0x82, 28, 1)]),
            KeyboardFormat::Boot => assert_eq!(endpoints(1), [(0x82, 8, 10)]),
        }
        assert_eq!(classes(2), (0x03, 0, 0));
        assert_eq!(endpoints(2), [(0x03, 64, 1), (0x83, 64, 1)]);
        assert_eq!(classes(3), (0x03, 0, 0));
        assert_eq!(endpoints(3), [(0x84, 8, 4)]);
        assert_eq!(classes(4), (0x03, 0, 0));
        assert_eq!(endpoints(4), [(0x85, 64, 1)]);
    }
}

#[test]
fn every_hid_interface_serves_the_report_descriptor_it_announces() {
    for face in faces() {
        let enumeration = enumerate(face).unwrap();
        let configuration = parse_configuration(&enumeration.configuration).unwrap();
        let expected_gamepad = match face.mode {
            Mode::Xinput => None,
            Mode::Gamepad => Some(GAMEPAD_DESCRIPTOR),
            Mode::NoGamepad => Some(PLACEHOLDER_DESCRIPTOR),
        };
        let expected_keyboard = match face.keyboard {
            KeyboardFormat::NKeyRollover => NKRO_DESCRIPTOR,
            KeyboardFormat::Boot => BOOT_KEYBOARD_DESCRIPTOR,
        };
        let expected = [
            expected_gamepad,
            Some(expected_keyboard),
            Some(CHANNEL_DESCRIPTOR),
            Some(CONTROLS_DESCRIPTOR),
            Some(KEY_STATE_DESCRIPTOR),
        ];
        for ((interface, served), expected) in configuration
            .interfaces
            .iter()
            .zip(&enumeration.report_descriptors)
            .zip(expected)
        {
            match expected {
                Some(descriptor) => {
                    assert_eq!(served.as_deref(), Ok(descriptor), "{face:?}");
                    assert_eq!(
                        interface.hid_report_descriptor_length(),
                        Some(u16::try_from(descriptor.len()).unwrap())
                    );
                    assert_eq!(interface.class_descriptors[0][2..4], [0x11, 0x01]);
                }
                None => assert!(served.is_err()),
            }
        }
    }
}

#[test]
fn device_descriptor_carries_the_ids_and_the_report_format() {
    for face in faces() {
        let device = enumerate(face).unwrap().device;
        let product_id = IDENTITY.ids.product_id_base + face.mode.product_id_offset();
        let release: u16 = match face.keyboard {
            KeyboardFormat::NKeyRollover => 0x0250,
            KeyboardFormat::Boot => 0x0251,
        };
        let [vendor_low, vendor_high] = IDENTITY.ids.vendor_id.to_le_bytes();
        let [product_low, product_high] = product_id.to_le_bytes();
        let [release_low, release_high] = release.to_le_bytes();
        assert_eq!(
            device,
            [
                18,
                1,
                0x00,
                0x02,
                0,
                0,
                0,
                64,
                vendor_low,
                vendor_high,
                product_low,
                product_high,
                release_low,
                release_high,
                1,
                2,
                3,
                1
            ]
        );
    }
}

#[test]
fn only_xinput_mode_answers_the_microsoft_os_descriptors() {
    let mut ms_os_string = vec![0x12, 0x03];
    for unit in "MSFT100\u{B0}".encode_utf16() {
        ms_os_string.extend(unit.to_le_bytes());
    }
    let mut compat_id = vec![
        0x28, 0, 0, 0, 0x00, 0x01, 0x04, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0,
    ];
    compat_id.extend([0x00, 0x01]);
    compat_id.extend(b"XUSB10\0\0");
    compat_id.extend([0; 14]);
    for face in faces() {
        let enumeration = enumerate(face).unwrap();
        if face.mode == Mode::Xinput {
            assert_eq!(
                enumeration.ms_os_string.as_deref(),
                Ok(ms_os_string.as_slice())
            );
            assert_eq!(
                enumeration.extended_compat_id.as_deref(),
                Ok(compat_id.as_slice())
            );
        } else {
            assert!(enumeration.ms_os_string.is_err());
            assert!(enumeration.extended_compat_id.is_err());
        }
    }
}

#[test]
fn default_face_is_full_keyboard_without_gamepad() {
    assert_eq!(
        Face::default(),
        Face {
            mode: Mode::NoGamepad,
            keyboard: KeyboardFormat::NKeyRollover,
            speed: Speed::High,
            gamepad_polling: PollingRate::Every1Millisecond,
        }
    );
}
