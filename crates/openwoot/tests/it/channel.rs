use std::pin::pin;

use embassy_futures::block_on;
use embassy_usb_driver::{EndpointError, Event};
use openwoot::{
    lighting::HostControl,
    protocol::{Answer, Channel, Handler},
    scan::SensorReadout,
    usb::{Face, FeatureRequests, HostState, Resources, build},
};
use openwoot_protocol::{
    ERROR_KEY, Endpoint, Error, FirmwareVersion, Key, PROTOCOL_VERSION, VarHeader, VarKey, VarSeq,
    Version, WireError, decode_message, decode_response, encode_message, encode_request,
    endpoints::{
        EnterUpdateMode, GetDiagnostics, GetLedFrame, GetSensorValues, GetVersion, Restart,
        SetRawScanning,
    },
    lighting::Effects,
    report,
    sensors::Reading,
    stock::{Command, Request},
};
use serde::Serialize;

use crate::{
    lighting::lighting,
    support::{Host, IDENTITY, Outcome, Recorder, poll, setup, stock_identity},
};

const CHANNEL_OUT: u8 = 0x03;
const CHANNEL_IN: u8 = 0x83;
const PACKET_SIZE: usize = 64;
const CHANNEL_INTERFACE: u16 = 2;
const CLASS_INTERFACE_OUT: u8 = 0x21;
const SET_REPORT: u8 = 0x09;
const SET_CONFIGURATION: u8 = 0x09;
const FEATURE_REPORT_1: u16 = 0x0301;
const INPUT_REPORT_LEN: usize = 33;
const HANDOVER_PAUSE_MS: u64 = 50;
const PACKET_GAP_MS: u64 = 100;
const PEER_TIMEOUT_MS: u64 = 500;
const VERSION: Version = Version {
    protocol: PROTOCOL_VERSION,
    firmware: FirmwareVersion {
        major: 1,
        minor: 2,
        patch: 3,
    },
};

fn exchange(request: &[u8]) -> (Vec<u8>, Recorder) {
    exchange_with(|host| host.send_packets(CHANNEL_OUT, request, PACKET_SIZE))
}

fn exchange_with(send: impl FnOnce(&Host)) -> (Vec<u8>, Recorder) {
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut channel = Channel::new(device.channel.output, device.channel.input);
    let mut handler = Handler::new(VERSION, stock_identity(None));
    let requests = FeatureRequests::new();
    let mut keyboard = Recorder::default();
    send(&host);
    assert_eq!(
        poll(pin!(channel.serve(&mut handler, &requests, &mut keyboard))),
        Some(Ok(()))
    );
    (host.take_sent(CHANNEL_IN).concat(), keyboard)
}

#[expect(
    clippy::unwrap_used,
    reason = "every request a test builds fits the largest report"
)]
fn encoded<E: Endpoint>(sequence: u16, request: &E::Request) -> Vec<u8>
where
    E::Request: Serialize,
{
    let mut report = [0; report::MAX_LEN];
    encode_request::<E>(sequence, request, &mut report)
        .unwrap()
        .to_vec()
}

fn stock_command(command: Command) -> Vec<u8> {
    Request::new(command, 0).to_report().to_vec()
}

fn feature_setup() -> [u8; 8] {
    setup(
        CLASS_INTERFACE_OUT,
        SET_REPORT,
        FEATURE_REPORT_1,
        CHANNEL_INTERFACE,
        8,
    )
}

#[test]
fn get_version_round_trips_through_the_channel_endpoints() {
    let mut request = [0; report::MAX_LEN];
    let request = encode_request::<GetVersion>(42, &(), &mut request).unwrap();
    let (response, keyboard) = exchange(request);
    assert_eq!(response.len(), 33);
    assert_eq!(response[..3], [1, b'O', b'W']);
    assert_eq!(
        decode_response::<GetVersion>(42, &response),
        Ok(Some(VERSION))
    );
    assert_eq!(keyboard.update_mode_options, []);
}

#[test]
fn enter_update_mode_answers_then_hands_over_to_the_bootloader_once() {
    let mut request = [0; report::MAX_LEN];
    let request = encode_request::<EnterUpdateMode>(5, &(), &mut request).unwrap();
    let (response, keyboard) = exchange(request);
    assert_eq!(
        decode_response::<EnterUpdateMode>(5, &response),
        Ok(Some(()))
    );
    assert_eq!(keyboard.update_mode_options, [0]);
    assert_eq!(keyboard.restart_options, []);
}

#[test]
fn restart_answers_then_hands_over_to_the_bootloader_once() {
    let mut request = [0; report::MAX_LEN];
    let request = encode_request::<Restart>(6, &(), &mut request).unwrap();
    let (response, keyboard) = exchange(request);
    assert_eq!(decode_response::<Restart>(6, &response), Ok(Some(())));
    assert_eq!(keyboard.restart_options, [0]);
    assert_eq!(keyboard.update_mode_options, []);
}

#[test]
fn an_unknown_endpoint_gets_an_error_reply() {
    let header = VarHeader {
        key: VarKey::Key8(Key::for_path::<u8>("openwoot/unknown")),
        seq_no: VarSeq::Seq2(9),
    };
    let mut request = [0; report::MAX_LEN];
    let request = encode_message(&header, &7_u8, &mut request).unwrap();
    let (response, keyboard) = exchange(request);
    assert_eq!(
        decode_response::<GetVersion>(9, &response),
        Err(Error::Device(WireError::UnknownKey))
    );
    assert_eq!(keyboard.update_mode_options, []);
}

#[test]
fn a_large_request_spans_many_packets() {
    let header = VarHeader {
        key: VarKey::Key8(Key::for_path::<u8>("openwoot/bulk")),
        seq_no: VarSeq::Seq2(1),
    };
    let mut request = [0; report::MAX_LEN];
    let body = [0xA5_u8; 1500];
    let request = encode_message(&header, &body[..], &mut request).unwrap();
    assert_eq!(request.len(), 2047);
    let (response, _) = exchange(request);
    assert_eq!(
        decode_response::<GetVersion>(1, &response),
        Err(Error::Device(WireError::UnknownKey))
    );
}

#[test]
fn stock_output_reports_get_no_answer_as_in_the_stock_firmware() {
    let mut request = [0_u8; 33];
    request[..4].copy_from_slice(&[1, 0xD1, 0xDA, 0x01]);
    let (response, keyboard) = exchange(&request);
    assert_eq!(response, Vec::<u8>::new());
    assert_eq!(keyboard.update_mode_options, []);
}

#[test]
fn enter_update_mode_hands_over_only_after_the_host_took_the_reply_and_a_pause() {
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut channel =
        Channel::with_delay(device.channel.output, device.channel.input, host.delay());
    let mut handler = Handler::new(VERSION, stock_identity(None));
    let requests = FeatureRequests::new();
    let mut keyboard = Recorder::default();
    host.stop_reading(CHANNEL_IN);
    host.send_packets(
        CHANNEL_OUT,
        &encoded::<EnterUpdateMode>(5, &()),
        PACKET_SIZE,
    );
    {
        let mut serve = pin!(channel.serve(&mut handler, &requests, &mut keyboard));
        assert_eq!(poll(serve.as_mut()), None);
        assert_eq!(host.sent_len(CHANNEL_IN), 0);
        host.resume_reading(CHANNEL_IN);
        assert_eq!(poll(serve.as_mut()), None);
        assert_eq!(host.sent_len(CHANNEL_IN), INPUT_REPORT_LEN);
        host.elapse_ms(HANDOVER_PAUSE_MS - 1);
        assert_eq!(poll(serve.as_mut()), None);
        host.elapse_ms(1);
        assert_eq!(poll(serve.as_mut()), Some(Ok(())));
    }
    assert_eq!(
        decode_response::<EnterUpdateMode>(5, &host.take_sent(CHANNEL_IN).concat()),
        Ok(Some(()))
    );
    assert_eq!(keyboard.update_mode_options, [0]);
}

#[test]
fn a_reply_that_cannot_be_sent_hands_nothing_over() {
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut channel = Channel::new(device.channel.output, device.channel.input);
    let mut handler = Handler::new(VERSION, stock_identity(None));
    let requests = FeatureRequests::new();
    let mut keyboard = Recorder::default();
    host.disable(CHANNEL_IN);
    host.send_packets(CHANNEL_OUT, &encoded::<Restart>(6, &()), PACKET_SIZE);
    assert_eq!(
        poll(pin!(channel.serve(&mut handler, &requests, &mut keyboard))),
        Some(Err(EndpointError::Disabled))
    );
    assert_eq!(keyboard.restart_options, []);
    assert_eq!(keyboard.update_mode_options, []);
}

#[test]
fn a_partial_transfer_is_dropped_after_the_packet_gap_and_the_next_request_is_answered() {
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut channel =
        Channel::with_delay(device.channel.output, device.channel.input, host.delay());
    let mut handler = Handler::new(VERSION, stock_identity(None));
    let requests = FeatureRequests::new();
    let mut keyboard = Recorder::default();
    let mut leftover = vec![3, b'O', b'W', 0xFF, 0x07];
    leftover.resize(PACKET_SIZE, 0xA5);
    host.send_raw_packets(CHANNEL_OUT, &[leftover]);
    {
        let mut serve = pin!(channel.serve(&mut handler, &requests, &mut keyboard));
        assert_eq!(poll(serve.as_mut()), None);
        host.elapse_ms(PACKET_GAP_MS);
        assert_eq!(poll(serve.as_mut()), None);
        host.send_packets(CHANNEL_OUT, &encoded::<GetVersion>(8, &()), PACKET_SIZE);
        assert_eq!(poll(serve.as_mut()), Some(Ok(())));
    }
    assert_eq!(
        decode_response::<GetVersion>(8, &host.take_sent(CHANNEL_IN).concat()),
        Ok(Some(VERSION))
    );
}

#[test]
fn a_report_ends_at_the_length_its_id_declares_even_in_a_full_packet() {
    let mut packet = encoded::<GetVersion>(9, &());
    assert_eq!(packet.len(), INPUT_REPORT_LEN);
    packet.resize(PACKET_SIZE, 0);
    let (response, keyboard) = exchange_with(|host| host.send_raw_packets(CHANNEL_OUT, &[packet]));
    assert_eq!(
        decode_response::<GetVersion>(9, &response),
        Ok(Some(VERSION))
    );
    assert_eq!(keyboard.update_mode_options, []);
}

#[test]
fn a_transfer_longer_than_the_largest_report_costs_no_later_request() {
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut channel = Channel::new(device.channel.output, device.channel.input);
    let mut handler = Handler::new(VERSION, stock_identity(None));
    let requests = FeatureRequests::new();
    let mut keyboard = Recorder::default();
    host.keep_oversized_packets();
    let mut junk = vec![vec![0xFF; PACKET_SIZE]; 33];
    junk.push(vec![0xFF]);
    host.send_raw_packets(CHANNEL_OUT, &junk);
    host.send_packets(CHANNEL_OUT, &encoded::<GetVersion>(10, &()), PACKET_SIZE);
    for _ in 0..3 {
        assert_eq!(
            poll(pin!(channel.serve(&mut handler, &requests, &mut keyboard))),
            Some(Ok(()))
        );
    }
    assert_eq!(host.pending_packets(CHANNEL_OUT), 0);
    let sent = host.take_sent(CHANNEL_IN);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        decode_response::<GetVersion>(10, &sent.concat()),
        Ok(Some(VERSION))
    );
}

#[test]
fn a_request_body_with_trailing_bytes_is_refused_and_hands_nothing_over() {
    let header = VarHeader {
        key: VarKey::Key8(EnterUpdateMode::REQ_KEY),
        seq_no: VarSeq::Seq2(11),
    };
    let mut request = [0; report::MAX_LEN];
    let request = encode_message(&header, &[0xFF_u8; 16], &mut request).unwrap();
    let (response, keyboard) = exchange(request);
    assert_eq!(
        decode_response::<EnterUpdateMode>(11, &response),
        Err(Error::Device(WireError::DeserFailed))
    );
    assert_eq!(keyboard.update_mode_options, []);
}

#[test]
fn a_reply_that_does_not_fit_the_response_buffer_hands_nothing_over() {
    let mut handler = Handler::new(VERSION, stock_identity(None));
    let request = encoded::<Restart>(12, &());
    let mut response = [0; 4];
    let answer = block_on(handler.respond(&request, &mut response));
    assert_eq!((answer.reply, answer.handover), (None, None));
}

#[test]
fn a_request_with_a_key_shorter_than_eight_bytes_is_unknown_even_when_it_abbreviates_an_endpoint() {
    let mut handler = Handler::new(VERSION, stock_identity(None));
    for shape in [
        &[0x00, 0x11, 7][..],
        &[0x40, 0x11, 0x22, 7],
        &[0x80, 0x11, 0x22, 0x33, 0x44, 7],
    ] {
        let mut shaped = [0; report::MAX_LEN];
        shaped[report::FRAME_OFFSET..][..shape.len()].copy_from_slice(shape);
        let shaped = report::seal(&mut shaped, shape.len()).unwrap();
        let header = decode_message(shaped)
            .unwrap()
            .unwrap()
            .reply_header(GetVersion::REQ_KEY);
        assert!(!matches!(header.key, VarKey::Key8(_)));
        let mut request = [0; report::MAX_LEN];
        let request = encode_message(&header, &(), &mut request).unwrap();
        let message = decode_message(request).unwrap().unwrap();
        let mut expected = [0; report::MAX_LEN];
        let expected = encode_message(
            &message.reply_header(ERROR_KEY),
            &WireError::UnknownKey,
            &mut expected,
        )
        .unwrap();
        let mut response = [0; report::MAX_LEN];
        let answer = block_on(handler.respond(request, &mut response));
        assert_eq!(
            answer,
            Answer {
                reply: Some(expected),
                handover: None
            }
        );
    }
}

#[test]
fn a_keyboard_without_diagnostics_refuses_their_key_before_reading_the_body() {
    let header = VarHeader {
        key: VarKey::Key8(GetDiagnostics::REQ_KEY),
        seq_no: VarSeq::Seq2(13),
    };
    let mut request = [0; report::MAX_LEN];
    let request = encode_message(&header, &[0xFF_u8; 4], &mut request).unwrap();
    let (response, _) = exchange(request);
    assert_eq!(
        decode_response::<GetDiagnostics>(13, &response),
        Err(Error::Device(WireError::UnknownKey))
    );
}

#[test]
fn a_bus_reset_ends_raw_scanning_host_lighting_and_gamepad_detection() {
    static STATE: HostState = HostState::new();
    static SENSORS: SensorReadout = SensorReadout::new();
    static HOST_CONTROL: HostControl = HostControl::new();
    let host = Host::default();
    let mut resources = Resources::new();
    let mut device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &STATE,
    );
    let mut channel = Channel::new(device.channel.output, device.channel.input);
    let mut run = pin!(device.usb.run());
    host.attach(run.as_mut());
    let configure = setup(0, SET_CONFIGURATION, 1, 0, 0);
    assert_eq!(
        host.control_out(run.as_mut(), configure, &[]),
        Some(Outcome::Accepted)
    );
    let mut handler = Handler::new(VERSION, stock_identity(None))
        .with_host(&STATE)
        .with_sensors(&SENSORS)
        .with_host_control(&HOST_CONTROL);
    let mut keyboard = Recorder::default();
    for command in [Command::EnterHostControl, Command::StartGamepadDetection] {
        assert_eq!(
            host.control_out(run.as_mut(), feature_setup(), &stock_command(command)),
            Some(Outcome::Accepted)
        );
    }
    {
        let mut serve = pin!(channel.serve(&mut handler, STATE.feature_requests(), &mut keyboard));
        assert_eq!(poll(serve.as_mut()), None);
        host.send_packets(
            CHANNEL_OUT,
            &encoded::<SetRawScanning>(1, &true),
            PACKET_SIZE,
        );
        assert_eq!(poll(serve.as_mut()), Some(Ok(())));
    }
    let mut lighting = lighting(255, Effects::new());
    HOST_CONTROL.apply(&mut lighting, 0);
    assert!(lighting.host_control());
    assert!(SENSORS.raw_scanning());
    assert!(STATE.gamepad_detection());

    host.bus_events(run.as_mut(), &[Event::Reset]);
    assert_eq!(
        host.control_out(run.as_mut(), configure, &[]),
        Some(Outcome::Accepted)
    );
    assert!(!STATE.gamepad_detection());
    host.send_packets(CHANNEL_OUT, &encoded::<GetVersion>(2, &()), PACKET_SIZE);
    assert_eq!(
        poll(pin!(channel.serve(
            &mut handler,
            STATE.feature_requests(),
            &mut keyboard
        ))),
        Some(Ok(()))
    );
    assert!(!SENSORS.raw_scanning());
    HOST_CONTROL.apply(&mut lighting, 0);
    assert!(!lighting.host_control());
}

#[test]
fn a_sensor_reading_nobody_publishes_fails_after_the_peer_time_out_and_frees_the_channel() {
    static SENSORS: SensorReadout = SensorReadout::new();
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut channel =
        Channel::with_delay(device.channel.output, device.channel.input, host.delay());
    let mut handler = Handler::new(VERSION, stock_identity(None)).with_sensors(&SENSORS);
    let requests = FeatureRequests::new();
    let mut keyboard = Recorder::default();
    host.send_packets(
        CHANNEL_OUT,
        &encoded::<GetSensorValues>(3, &Reading::Travel),
        PACKET_SIZE,
    );
    {
        let mut serve = pin!(channel.serve(&mut handler, &requests, &mut keyboard));
        assert_eq!(poll(serve.as_mut()), None);
        host.elapse_ms(PEER_TIMEOUT_MS);
        assert_eq!(poll(serve.as_mut()), Some(Ok(())));
    }
    assert_eq!(
        decode_response::<GetSensorValues>(3, &host.take_sent(CHANNEL_IN).concat()),
        Err(Error::Device(WireError::FailedToSpawn))
    );
    host.send_packets(
        CHANNEL_OUT,
        &encoded::<EnterUpdateMode>(4, &()),
        PACKET_SIZE,
    );
    {
        let mut serve = pin!(channel.serve(&mut handler, &requests, &mut keyboard));
        assert_eq!(poll(serve.as_mut()), None);
        host.elapse_ms(HANDOVER_PAUSE_MS);
        assert_eq!(poll(serve.as_mut()), Some(Ok(())));
    }
    assert_eq!(keyboard.update_mode_options, [0]);
}

#[test]
fn a_led_frame_nobody_renders_fails_after_the_peer_time_out_and_frees_the_channel() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut channel =
        Channel::with_delay(device.channel.output, device.channel.input, host.delay());
    let mut handler = Handler::new(VERSION, stock_identity(None)).with_host_control(&HOST_CONTROL);
    let requests = FeatureRequests::new();
    let mut keyboard = Recorder::default();
    host.send_packets(CHANNEL_OUT, &encoded::<GetLedFrame>(5, &()), PACKET_SIZE);
    {
        let mut serve = pin!(channel.serve(&mut handler, &requests, &mut keyboard));
        assert_eq!(poll(serve.as_mut()), None);
        host.elapse_ms(PEER_TIMEOUT_MS - 1);
        assert_eq!(poll(serve.as_mut()), None);
        assert_eq!(host.sent_len(CHANNEL_IN), 0);
        host.elapse_ms(1);
        assert_eq!(poll(serve.as_mut()), Some(Ok(())));
    }
    assert_eq!(
        decode_response::<GetLedFrame>(5, &host.take_sent(CHANNEL_IN).concat()),
        Err(Error::Device(WireError::FailedToSpawn))
    );
    host.send_packets(CHANNEL_OUT, &encoded::<GetVersion>(6, &()), PACKET_SIZE);
    assert_eq!(
        poll(pin!(channel.serve(&mut handler, &requests, &mut keyboard))),
        Some(Ok(()))
    );
    assert_eq!(
        decode_response::<GetVersion>(6, &host.take_sent(CHANNEL_IN).concat()),
        Ok(Some(VERSION))
    );
}

#[test]
fn a_sensor_request_that_does_not_decode_is_refused_without_waiting_for_a_reading() {
    static SENSORS: SensorReadout = SensorReadout::new();
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut channel =
        Channel::with_delay(device.channel.output, device.channel.input, host.delay());
    let mut handler = Handler::new(VERSION, stock_identity(None)).with_sensors(&SENSORS);
    let requests = FeatureRequests::new();
    let mut keyboard = Recorder::default();
    let header = VarHeader {
        key: VarKey::Key8(GetSensorValues::REQ_KEY),
        seq_no: VarSeq::Seq2(7),
    };
    let mut request = [0; report::MAX_LEN];
    let request = encode_message(&header, &[0xFF_u8; 8], &mut request).unwrap();
    host.send_packets(CHANNEL_OUT, request, PACKET_SIZE);
    assert_eq!(
        poll(pin!(channel.serve(&mut handler, &requests, &mut keyboard))),
        Some(Ok(()))
    );
    assert_eq!(
        decode_response::<GetSensorValues>(7, &host.take_sent(CHANNEL_IN).concat()),
        Err(Error::Device(WireError::DeserFailed))
    );
}

#[test]
fn a_stock_lighting_command_the_lighting_task_cannot_take_is_answered_busy() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let host = Host::default();
    let state = HostState::new();
    let mut resources = Resources::new();
    let mut device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        &state,
    );
    let mut channel =
        Channel::with_delay(device.channel.output, device.channel.input, host.delay());
    let mut run = pin!(device.usb.run());
    host.attach(run.as_mut());
    let mut handler = Handler::new(VERSION, stock_identity(None)).with_host_control(&HOST_CONTROL);
    let mut keyboard = Recorder::default();
    let mut statuses = Vec::new();
    {
        let mut serve = pin!(channel.serve(&mut handler, state.feature_requests(), &mut keyboard));
        for _ in 0..17 {
            assert_eq!(
                host.control_out(
                    run.as_mut(),
                    feature_setup(),
                    &stock_command(Command::EnterHostControl)
                ),
                Some(Outcome::Accepted)
            );
            assert_eq!(poll(serve.as_mut()), None);
            if host.sent_len(CHANNEL_IN) == 0 {
                host.elapse_ms(PEER_TIMEOUT_MS);
                assert_eq!(poll(serve.as_mut()), None);
            }
            statuses.push(host.take_sent(CHANNEL_IN).concat()[4]);
        }
    }
    let mut expected = vec![0x88; 15];
    expected.extend([0x77, 0x77]);
    assert_eq!(statuses, expected);
}
