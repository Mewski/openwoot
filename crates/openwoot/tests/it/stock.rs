use std::pin::pin;

use openwoot::{
    protocol::{Channel, Handler, StockIdentity},
    usb::{Face, HostState, Resources, build},
};
use openwoot_protocol::{FirmwareVersion, PROTOCOL_VERSION, Version};

use crate::support::{
    EXAMPLE_SERIAL, Host, IDENTITY, Outcome, Recorder, poll, setup, stock_identity,
};

const CHANNEL_IN: u8 = 0x83;
const CHANNEL_INTERFACE: u16 = 2;
const CLASS_INTERFACE_OUT: u8 = 0x21;
const CLASS_INTERFACE_IN: u8 = 0xA1;
const GET_REPORT: u8 = 0x01;
const SET_REPORT: u8 = 0x09;
const FEATURE_REPORT_1: u16 = 0x0301;
const OUTPUT_REPORT_1: u16 = 0x0201;
const INPUT_REPORT_LEN: usize = 33;
const VERSION: Version = Version {
    protocol: PROTOCOL_VERSION,
    firmware: FirmwareVersion {
        major: 0,
        minor: 1,
        patch: 0,
    },
};

struct Exchange {
    outcomes: Vec<Option<Outcome>>,
    replies: Vec<Vec<u8>>,
    keyboard: Recorder,
}

fn feature(command: u8, argument: u32) -> Vec<u8> {
    let mut report = vec![1, 0xD1, 0xDA, command];
    report.extend(argument.to_le_bytes());
    report
}

fn input_report(frame: &[u8]) -> Vec<u8> {
    let mut report = vec![1];
    report.extend(frame);
    report.resize(INPUT_REPORT_LEN, 0);
    report
}

fn set_reports(identity: StockIdentity, reports: &[(u16, Vec<u8>)]) -> Exchange {
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
    let mut run = pin!(device.usb.run());
    host.attach(run.as_mut());
    let mut channel = Channel::new(device.channel.output, device.channel.input);
    let mut handler = Handler::new(VERSION, identity);
    let mut keyboard = Recorder::default();
    let mut outcomes = Vec::new();
    let mut replies = Vec::new();
    for (value, report) in reports {
        let length = u16::try_from(report.len()).unwrap_or(u16::MAX);
        outcomes.push(host.control_out(
            run.as_mut(),
            setup(
                CLASS_INTERFACE_OUT,
                SET_REPORT,
                *value,
                CHANNEL_INTERFACE,
                length,
            ),
            report,
        ));
        let _ = poll(pin!(channel.serve(
            &mut handler,
            state.feature_requests(),
            &mut keyboard
        )));
        replies.push(host.take_sent(CHANNEL_IN).concat());
    }
    Exchange {
        outcomes,
        replies,
        keyboard,
    }
}

fn answer(identity: StockIdentity, command: u8, argument: u32) -> Vec<u8> {
    let exchange = set_reports(identity, &[(FEATURE_REPORT_1, feature(command, argument))]);
    assert_eq!(exchange.outcomes, [Some(Outcome::Accepted)]);
    assert_eq!(exchange.keyboard.update_mode_options, []);
    exchange.replies.concat()
}

#[test]
fn ping_answers_the_inverted_low_argument_byte_as_protocol_cmd_ping() {
    assert_eq!(
        answer(stock_identity(None), 0, 0),
        input_report(&[0xD1, 0xDA, 0x00, 0x88, 0x01, 0x00, 0xFF])
    );
    assert_eq!(
        answer(stock_identity(None), 0, 0x1234_565A),
        input_report(&[0xD1, 0xDA, 0x00, 0x88, 0x01, 0x00, 0xA5])
    );
}

#[test]
fn version_answers_a_production_version_so_that_wootility_requires_the_update() {
    assert_eq!(
        answer(stock_identity(None), 1, 0),
        input_report(&[0xD1, 0xDA, 0x01, 0x88, 0x03, 0x00, 0x02, 0x01, 0x02])
    );
}

#[test]
fn serial_selector_0_answers_the_legacy_block_of_protocol_get_serial_legacy() {
    assert_eq!(
        answer(stock_identity(Some(EXAMPLE_SERIAL)), 3, 0),
        input_report(&[
            0xD1, 0xDA, 0x03, 0x88, 0x0B, 0x00, 0x01, 0x00, 0x18, 0x01, 0x01, 0x00, 0x01, 0x00,
            0x2C, 0x01, 0x00
        ])
    );
}

#[test]
fn serial_selector_0_without_a_serial_answers_zeros_as_protocol_get_serial_legacy() {
    let mut frame = vec![0xD1, 0xDA, 0x03, 0x88, 0x0B, 0x00];
    frame.extend([0; 11]);
    assert_eq!(answer(stock_identity(None), 3, 0), input_report(&frame));
}

#[test]
fn serial_selector_2_answers_the_serial_number_message_as_nanopb_encodes_it() {
    assert_eq!(
        answer(stock_identity(Some(EXAMPLE_SERIAL)), 3, 2),
        input_report(&[
            0xD1, 0xDA, 0x03, 0x88, 0x15, 0x00, 0x08, 0x01, 0x10, 0x18, 0x18, 0x01, 0x20, 0x01,
            0x28, 0x01, 0x30, 0xAC, 0x02, 0x38, 0x00, 0x48, 0x01, 0x50, 0x00, 0x58, 0x00
        ])
    );
}

#[test]
fn serial_selector_2_without_a_serial_answers_the_blank_record_error_minus_2() {
    assert_eq!(
        answer(stock_identity(None), 3, 2),
        input_report(&[0xD1, 0xDA, 0x03, 0xFF, 0x02, 0x00, 0xFE, 0xFF])
    );
}

#[test]
fn serial_selectors_1_and_3_answer_their_lengths_and_others_an_error() {
    let mut mcu_id = vec![0xD1, 0xDA, 0x03, 0x88, 0x0C, 0x00];
    mcu_id.extend([0; 12]);
    assert_eq!(answer(stock_identity(None), 3, 1), input_report(&mcu_id));
    let mut flash_ids = vec![0xD1, 0xDA, 0x03, 0x88, 0x0A, 0x00];
    flash_ids.extend([0; 10]);
    assert_eq!(answer(stock_identity(None), 3, 3), input_report(&flash_ids));
    assert_eq!(
        answer(stock_identity(None), 3, 4),
        input_report(&[0xD1, 0xDA, 0x03, 0xFF, 0x00, 0x00])
    );
}

#[test]
fn device_settings_answer_the_face_unprotected_flash_and_the_first_layout() {
    assert_eq!(
        answer(stock_identity(None), 19, 0),
        input_report(&[
            0xD1, 0xDA, 0x13, 0x88, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11, 0x00, 0x0C
        ])
    );
}

#[test]
fn other_commands_get_the_stock_unknown_or_unsupported_status() {
    for (command, status) in [
        (4, 0x66),
        (7, 0x66),
        (25, 0x66),
        (37, 0x66),
        (38, 0xAA),
        (60, 0xAA),
        (81, 0xAA),
        (88, 0xAA),
        (89, 0x66),
        (255, 0x66),
    ] {
        assert_eq!(
            answer(stock_identity(None), command, 0),
            input_report(&[0xD1, 0xDA, command, status, 0x00, 0x00]),
            "command {command}"
        );
    }
}

#[test]
fn a_wrong_magic_gets_the_error_status_under_the_stock_magic() {
    let exchange = set_reports(
        stock_identity(None),
        &[(FEATURE_REPORT_1, vec![1, 0xD1, 0xDB, 0x01, 0, 0, 0, 0])],
    );
    assert_eq!(
        exchange.replies.concat(),
        input_report(&[0xD1, 0xDA, 0x01, 0xFF, 0x00, 0x00])
    );
}

#[test]
fn a_feature_report_without_its_id_byte_is_answered_too() {
    let exchange = set_reports(
        stock_identity(None),
        &[(FEATURE_REPORT_1, vec![0xD1, 0xDA, 0x00, 0x01, 0, 0, 0])],
    );
    assert_eq!(
        exchange.replies.concat(),
        input_report(&[0xD1, 0xDA, 0x00, 0x88, 0x01, 0x00, 0xFE])
    );
}

#[test]
fn reset_to_update_mode_hands_the_argument_byte_over_once_without_a_reply() {
    let exchange = set_reports(
        stock_identity(None),
        &[(FEATURE_REPORT_1, feature(2, 0x0000_0105))],
    );
    assert_eq!(exchange.outcomes, [Some(Outcome::Accepted)]);
    assert_eq!(exchange.replies.concat(), Vec::<u8>::new());
    assert_eq!(exchange.keyboard.update_mode_options, [0x05]);
}

#[test]
fn wootility_connect_sequence_is_answered_in_order() {
    let sequence = [0, 3, 3, 19, 1, 19, 3, 3, 19];
    let arguments = [0, 2, 0, 0, 0, 0, 1, 3, 0];
    let reports: Vec<_> = sequence
        .iter()
        .zip(arguments)
        .map(|(&command, argument)| (FEATURE_REPORT_1, feature(command, argument)))
        .collect();
    let exchange = set_reports(stock_identity(Some(EXAMPLE_SERIAL)), &reports);
    assert!(
        exchange
            .outcomes
            .iter()
            .all(|outcome| *outcome == Some(Outcome::Accepted))
    );
    for (reply, command) in exchange.replies.iter().zip(sequence) {
        assert_eq!(reply.len(), INPUT_REPORT_LEN);
        assert_eq!(reply[..5], [1, 0xD1, 0xDA, command, 0x88]);
    }
}

#[test]
fn output_reports_through_the_control_endpoint_are_accepted_and_ignored() {
    let mut report = vec![1, 0xD1, 0xDA, 0x07];
    report.resize(INPUT_REPORT_LEN, 0);
    let exchange = set_reports(stock_identity(None), &[(OUTPUT_REPORT_1, report)]);
    assert_eq!(exchange.outcomes, [Some(Outcome::Accepted)]);
    assert_eq!(exchange.replies.concat(), Vec::<u8>::new());
}

#[test]
fn get_report_answers_the_report_id_byte_only_as_stock() {
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
    let mut run = pin!(device.usb.run());
    host.attach(run.as_mut());
    assert_eq!(
        host.control_in(
            run.as_mut(),
            setup(
                CLASS_INTERFACE_IN,
                GET_REPORT,
                FEATURE_REPORT_1,
                CHANNEL_INTERFACE,
                8
            )
        ),
        Ok(vec![1])
    );
    assert_eq!(
        host.control_in(
            run.as_mut(),
            setup(
                CLASS_INTERFACE_IN,
                GET_REPORT,
                0x0106,
                CHANNEL_INTERFACE,
                64
            )
        ),
        Ok(vec![6])
    );
    assert_eq!(
        host.control_in(
            run.as_mut(),
            setup(CLASS_INTERFACE_IN, GET_REPORT, 0x0300, CHANNEL_INTERFACE, 8)
        ),
        Err(Outcome::Rejected)
    );
}
