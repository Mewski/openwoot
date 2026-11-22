use std::{
    future::Future,
    pin::{Pin, pin},
};

use embassy_futures::join::join3;
use embassy_usb_driver::Speed as BusSpeed;
use openwoot::{
    hid::{
        CHANNEL_DESCRIPTOR, CONTROLS_DESCRIPTOR, ConsumerReport, GAMEPAD_DESCRIPTOR, GamepadReport,
        InputReport, KEY_STATE_DESCRIPTOR, KeyStateReport, MouseReport, NKRO_DESCRIPTOR,
        NkroReport, PLACEHOLDER_DESCRIPTOR, PLACEHOLDER_LEN, SystemReport,
    },
    protocol::Handler,
    runtime::{self, Usb},
    usb::{Face, HostState, KeyboardFormat, Mode, PollingRate, Resources, Speed},
};
use openwoot_protocol::{GetVersion, MAX_REPORT_LEN, decode_response, encode_request};

use crate::support::{
    EXAMPLE_SERIAL, Host, IDENTITY, Outcome, Recorder, parse_configuration, poll, setup,
    stock_identity,
};

const GET_DESCRIPTOR: u8 = 0x06;
const SET_CONFIGURATION: u8 = 0x09;
const DEVICE_TO_HOST: u8 = 0x80;
const INTERFACE_RECIPIENT: u8 = 0x01;
const GAMEPAD_IN: u8 = 0x81;
const KEYBOARD_IN: u8 = 0x82;
const CHANNEL_OUT: u8 = 0x03;
const CHANNEL_IN: u8 = 0x83;
const CONTROLS_IN: u8 = 0x84;
const KEY_STATE_IN: u8 = 0x85;
const CHANNEL_PACKET_SIZE: usize = 64;

struct Session {
    device: Vec<u8>,
    configuration: Vec<u8>,
    report_descriptors: Vec<Result<Vec<u8>, Outcome>>,
    configured: Option<Outcome>,
    idle_reports: Vec<(u8, Vec<Vec<u8>>)>,
    version_response: Vec<u8>,
}

const HIGH_SPEED: fn() -> Option<BusSpeed> = || Some(BusSpeed::High);
const FULL_SPEED: fn() -> Option<BusSpeed> = || Some(BusSpeed::Full);
const BEFORE_RESET: fn() -> Option<BusSpeed> = || None;

fn connect(
    face: Face,
    negotiated_speed: fn() -> Option<BusSpeed>,
    version_request: &[u8],
) -> Result<Session, Outcome> {
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let usb = Usb::new(
        host.driver(),
        negotiated_speed,
        face,
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut keyboard = Recorder::default();
    let handler = Handler::new(
        runtime::version("1", "2", "3"),
        stock_identity(Some(EXAMPLE_SERIAL)),
    );
    let Usb {
        mut device,
        configuration,
        reports,
    } = usb;
    let mut firmware = pin!(join3(
        device.run(),
        configuration.run(handler, &mut keyboard),
        reports.run(),
    ));
    host.attach(firmware.as_mut());
    let get = |firmware: Pin<&mut _>, request_type: u8, value: u16, index: u16| {
        host.control_in(
            firmware,
            setup(request_type, GET_DESCRIPTOR, value, index, 0xFFFF),
        )
    };
    let device_descriptor = get(firmware.as_mut(), DEVICE_TO_HOST, 0x0100, 0)?;
    let configuration = get(firmware.as_mut(), DEVICE_TO_HOST, 0x0200, 0)?;
    let report_descriptors = (0..5)
        .map(|interface| {
            get(
                firmware.as_mut(),
                DEVICE_TO_HOST | INTERFACE_RECIPIENT,
                0x2200,
                interface,
            )
        })
        .collect();
    let configured = host.control_out(firmware.as_mut(), setup(0, SET_CONFIGURATION, 1, 0, 0), &[]);
    settle(firmware.as_mut());
    let idle_reports = [GAMEPAD_IN, KEYBOARD_IN, CONTROLS_IN, KEY_STATE_IN]
        .into_iter()
        .map(|endpoint| (endpoint, host.take_sent(endpoint)))
        .collect();
    if !version_request.is_empty() {
        host.send_packets(CHANNEL_OUT, version_request, CHANNEL_PACKET_SIZE);
    }
    settle(firmware.as_mut());
    Ok(Session {
        device: device_descriptor,
        configuration,
        report_descriptors,
        configured,
        idle_reports,
        version_response: host.take_sent(CHANNEL_IN).concat(),
    })
}

fn settle<F: Future>(firmware: Pin<&mut F>) {
    assert!(poll(firmware).is_none());
}

fn gamepad_face(polling: PollingRate) -> Face {
    Face {
        mode: Mode::Gamepad,
        keyboard: KeyboardFormat::NKeyRollover,
        speed: Speed::High,
        gamepad_polling: polling,
    }
}

#[test]
fn the_firmware_enumerates_and_answers_get_version_at_both_speeds() {
    let mut request = [0; MAX_REPORT_LEN];
    let request = encode_request::<GetVersion>(7, &(), &mut request).unwrap();
    for negotiated_speed in [HIGH_SPEED, FULL_SPEED] {
        let session = connect(Face::default(), negotiated_speed, request).unwrap();
        assert_eq!(session.device[..2], [18, 1]);
        assert_eq!(
            u16::from_le_bytes([session.device[10], session.device[11]]),
            IDENTITY.ids.product_id_base + Mode::NoGamepad.product_id_offset()
        );
        let configuration = parse_configuration(&session.configuration).unwrap();
        assert_eq!(configuration.interface_count, 5);
        let expected = [
            PLACEHOLDER_DESCRIPTOR,
            NKRO_DESCRIPTOR,
            CHANNEL_DESCRIPTOR,
            CONTROLS_DESCRIPTOR,
            KEY_STATE_DESCRIPTOR,
        ];
        for (served, expected) in session.report_descriptors.iter().zip(expected) {
            assert_eq!(served.as_deref(), Ok(expected));
        }
        assert_eq!(session.configured, Some(Outcome::Accepted));
        let version = decode_response::<GetVersion>(7, &session.version_response)
            .unwrap()
            .unwrap();
        assert_eq!(version, runtime::version("1", "2", "3"));
    }
}

#[test]
fn the_gamepad_interval_follows_the_negotiated_speed() {
    for (polling, negotiated_speed, interval) in [
        (PollingRate::Every1Millisecond, HIGH_SPEED, 4),
        (PollingRate::Every1Millisecond, FULL_SPEED, 1),
        (PollingRate::Every4Milliseconds, HIGH_SPEED, 6),
        (PollingRate::Every4Milliseconds, FULL_SPEED, 4),
        (PollingRate::Every125Microseconds, FULL_SPEED, 1),
        (PollingRate::Every4Milliseconds, BEFORE_RESET, 6),
    ] {
        let session = connect(gamepad_face(polling), negotiated_speed, &[]).unwrap();
        let configuration = parse_configuration(&session.configuration).unwrap();
        let gamepad = &configuration.interfaces[0].endpoints[0];
        assert_eq!(gamepad.address, GAMEPAD_IN);
        assert_eq!(gamepad.interval, interval, "{polling:?}");
        assert_eq!(
            session.report_descriptors[0].as_deref(),
            Ok(GAMEPAD_DESCRIPTOR)
        );
        let keyboard = &configuration.interfaces[1].endpoints[0];
        assert_eq!(keyboard.interval, 1);
    }
}

#[test]
fn every_input_endpoint_sends_its_idle_report_once_configured() {
    let session = connect(Face::default(), FULL_SPEED, &[]).unwrap();
    assert_eq!(
        session.idle_reports,
        [
            (GAMEPAD_IN, vec![vec![0; PLACEHOLDER_LEN]]),
            (KEYBOARD_IN, packets(&NkroReport::new(), 28)),
            (
                CONTROLS_IN,
                [
                    SystemReport::default().to_bytes().as_ref().to_vec(),
                    ConsumerReport::default().to_bytes().as_ref().to_vec(),
                    MouseReport::default().to_bytes().as_ref().to_vec(),
                ]
                .to_vec()
            ),
            (KEY_STATE_IN, packets(&KeyStateReport::default(), 64)),
        ]
    );
    let gamepad = connect(
        gamepad_face(PollingRate::Every1Millisecond),
        HIGH_SPEED,
        &[],
    )
    .unwrap();
    assert_eq!(
        gamepad.idle_reports[0],
        (GAMEPAD_IN, packets(&GamepadReport::new(), 32))
    );
}

fn packets<Report: InputReport>(report: &Report, packet_size: usize) -> Vec<Vec<u8>> {
    report
        .to_bytes()
        .as_ref()
        .chunks(packet_size)
        .map(<[u8]>::to_vec)
        .collect()
}
