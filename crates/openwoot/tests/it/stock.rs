use std::pin::pin;

use embassy_futures::block_on;
use openwoot::{
    keys::Key,
    lighting::{HostControl, from_rgb565, rgb},
    protocol::{Channel, Configuration, Handler, StockIdentity},
    storage::NorIdentity,
    usb::{Face, HostState, Resources, build},
};
use openwoot_drivers::spi_nor::Identification;
use openwoot_protocol::{FirmwareVersion, PROTOCOL_VERSION, Version, lighting::Effects, report};

use crate::{
    lighting::{COLUMNS, MAP, ROWS, VARIANT, base_color, lighting},
    support::{
        EXAMPLE_SERIAL, Host, IDENTITY, KeyMatrix, Outcome, Recorder, poll, setup, stock_identity,
    },
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
    exchange(
        &mut Handler::new(VERSION, identity),
        &HostState::new(),
        reports,
    )
}

fn exchange<C: Configuration>(
    handler: &mut Handler<C>,
    state: &HostState,
    reports: &[(u16, Vec<u8>)],
) -> Exchange {
    let host = Host::default();
    let mut resources = Resources::new();
    let mut device = build(
        host.driver(),
        Face::default(),
        IDENTITY,
        &mut resources,
        state,
    );
    let mut run = pin!(device.usb.run());
    host.attach(run.as_mut());
    let mut channel = Channel::new(device.channel.output, device.channel.input);
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
        assert_eq!(
            poll(pin!(channel.serve(
                handler,
                state.feature_requests(),
                &mut keyboard
            ))),
            None
        );
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
fn the_layout_and_the_hardware_ids_come_from_the_identity() {
    let identity = StockIdentity {
        layout: 2,
        mcu_id: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        nor_identity: Some(NorIdentity {
            identification: Identification {
                manufacturer: 0xEF,
                device: 0x16,
            },
            unique_id: [0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28],
        }),
        ..stock_identity(None)
    };
    assert_eq!(
        answer(identity, 19, 0),
        input_report(&[
            0xD1, 0xDA, 0x13, 0x88, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x11, 0x02, 0x0C
        ])
    );
    let mut mcu_id = vec![0xD1, 0xDA, 0x03, 0x88, 0x0C, 0x00];
    mcu_id.extend(identity.mcu_id);
    assert_eq!(answer(identity, 3, 1), input_report(&mcu_id));
    let mut flash_ids = vec![0xD1, 0xDA, 0x03, 0x88, 0x0A, 0x00];
    flash_ids.extend([0xEF, 0x16, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28]);
    assert_eq!(answer(identity, 3, 3), input_report(&flash_ids));
}

#[test]
fn other_commands_get_the_stock_unknown_or_unsupported_status() {
    for (command, status) in [
        (4, 0x66),
        (7, 0x66),
        (21, 0x66),
        (25, 0x66),
        (33, 0x66),
        (37, 0x66),
        (38, 0xAA),
        (46, 0xAA),
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

fn empty_reply(command: u8, status: u8) -> Vec<u8> {
    input_report(&[0xD1, 0xDA, command, status, 0x00, 0x00])
}

fn led_owner(host_control: &'static HostControl) -> Handler<KeyMatrix> {
    Handler::with_configuration(VERSION, stock_identity(None), KeyMatrix)
        .with_host_control(host_control)
}

#[test]
fn keys_off_and_on_switch_the_key_reports_as_protocol_cmd_keys_off_and_on() {
    static STATE: HostState = HostState::new();
    let mut handler = Handler::new(VERSION, stock_identity(None)).with_host(&STATE);
    let off = exchange(&mut handler, &STATE, &[(FEATURE_REPORT_1, feature(21, 0))]);
    assert_eq!(off.replies.concat(), empty_reply(0x15, 0x88));
    assert!(!STATE.key_reports());
    let on = exchange(&mut handler, &STATE, &[(FEATURE_REPORT_1, feature(22, 0))]);
    assert_eq!(on.replies.concat(), empty_reply(0x16, 0x88));
    assert!(STATE.key_reports());
}

#[test]
fn xinput_detection_start_and_stop_switch_the_gamepad_detection_pattern() {
    static STATE: HostState = HostState::new();
    let mut handler = Handler::new(VERSION, stock_identity(None)).with_host(&STATE);
    let start = exchange(&mut handler, &STATE, &[(FEATURE_REPORT_1, feature(45, 0))]);
    assert_eq!(start.replies.concat(), empty_reply(0x2D, 0x88));
    assert!(STATE.gamepad_detection());
    let stop = exchange(&mut handler, &STATE, &[(FEATURE_REPORT_1, feature(46, 0))]);
    assert_eq!(stop.replies.concat(), empty_reply(0x2E, 0x88));
    assert!(!STATE.gamepad_detection());
}

#[test]
fn sdk_set_key_color_takes_the_leds_and_paints_one_key_as_protocol_cmd_sdk_set_key_color() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = led_owner(&HOST_CONTROL);
    let exchange = exchange(
        &mut handler,
        &HostState::new(),
        &[
            (FEATURE_REPORT_1, feature(30, 0x43FF_8000)),
            (FEATURE_REPORT_1, feature(30, 0xFF12_3456)),
            (FEATURE_REPORT_1, feature(30, 0x1100_0000)),
        ],
    );
    assert_eq!(
        exchange.replies,
        [
            empty_reply(0x1E, 0x88),
            empty_reply(0x1E, 0xFF),
            empty_reply(0x1E, 0xFF)
        ]
    );
    let mut lighting = lighting(255, Effects::new());
    HOST_CONTROL.apply(&mut lighting, 0);
    assert!(lighting.host_control());
    assert_eq!(lighting.key_color(Key::new(2, 3)), Some(rgb(255, 128, 0)));
}

#[test]
fn sdk_reset_key_and_reset_colors_give_the_leds_back_as_protocol_rgb_sdk() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = led_owner(&HOST_CONTROL);
    let key = Key::new(2, 3);
    let exchange = exchange(
        &mut handler,
        &HostState::new(),
        &[
            (FEATURE_REPORT_1, feature(33, 0)),
            (FEATURE_REPORT_1, feature(30, 0x43FF_8000)),
            (FEATURE_REPORT_1, feature(31, 0x43)),
            (FEATURE_REPORT_1, feature(31, 0xFF)),
        ],
    );
    assert_eq!(
        exchange.replies,
        [
            empty_reply(0x21, 0x88),
            empty_reply(0x1E, 0x88),
            empty_reply(0x1F, 0x88),
            empty_reply(0x1F, 0xFF)
        ]
    );
    let mut lighting = lighting(255, Effects::new());
    HOST_CONTROL.apply(&mut lighting, 0);
    assert!(lighting.host_control());
    assert_eq!(lighting.key_color(key), Some(from_rgb565(base_color(2, 3))));
    let leave = self::exchange(
        &mut handler,
        &HostState::new(),
        &[(FEATURE_REPORT_1, feature(32, 0))],
    );
    assert_eq!(leave.replies.concat(), empty_reply(0x20, 0x88));
    HOST_CONTROL.apply(&mut lighting, 0);
    assert!(!lighting.host_control());
}

#[test]
fn sdk_set_all_colors_output_report_paints_every_key_from_the_21_column_map() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = led_owner(&HOST_CONTROL);
    let mut request = vec![4, 0xD1, 0xDA, 0x0B];
    for row in 0..6 {
        for column in 0..21 {
            let color = if column < COLUMNS {
                base_color(5 - row, COLUMNS - 1 - column)
            } else {
                0xFFFF
            };
            request.extend(color.to_le_bytes());
        }
    }
    request.resize(511, 0);
    let mut response = [0; report::MAX_LEN];
    let answer = block_on(handler.respond(&request, &mut response));
    assert_eq!((answer.reply, answer.handover), (None, None));
    let mut lighting = lighting(255, Effects::new());
    HOST_CONTROL.apply(&mut lighting, 0);
    assert!(lighting.host_control());
    let mut painted = 0;
    let mut with_led = 0;
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let key = Key::new(u8::try_from(row).unwrap(), u8::try_from(column).unwrap());
            if let Some(color) = lighting.key_color(key) {
                let expected = from_rgb565(base_color(5 - row, COLUMNS - 1 - column));
                assert_eq!(color, expected, "row {row} column {column}");
                painted += 1;
            }
            if VARIANT
                .led(row, column)
                .and_then(|led| MAP.position(led))
                .is_some()
            {
                with_led += 1;
            }
        }
    }
    assert!(with_led > 0);
    assert_eq!(painted, with_led);
}

#[test]
fn a_short_sdk_set_all_colors_report_is_ignored() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = led_owner(&HOST_CONTROL);
    let mut request = vec![1, 0xD1, 0xDA, 0x0B];
    request.resize(33, 0);
    let mut response = [0; report::MAX_LEN];
    let _ = block_on(handler.respond(&request, &mut response));
    let mut lighting = lighting(255, Effects::new());
    HOST_CONTROL.apply(&mut lighting, 0);
    assert!(!lighting.host_control());
}

#[test]
fn reset_to_update_mode_needs_feature_report_1_with_at_least_its_seven_bytes() {
    let exchange = set_reports(
        stock_identity(None),
        &[
            (0x0300, feature(2, 0)),
            (0x0300, feature(2, 0)[1..].to_vec()),
            (0x0307, feature(2, 0)),
            (0x03FF, feature(2, 0)),
            (FEATURE_REPORT_1, vec![1, 0xD1, 0xDA, 2]),
            (FEATURE_REPORT_1, vec![0xD1, 0xDA, 2]),
            (FEATURE_REPORT_1, Vec::new()),
            (FEATURE_REPORT_1, vec![1, 0xDA, 0xD1, 2, 0, 0, 0, 0]),
            (OUTPUT_REPORT_1, feature(2, 0)),
        ],
    );
    let mut expected = vec![Some(Outcome::Rejected); 7];
    expected.extend([Some(Outcome::Accepted); 2]);
    assert_eq!(exchange.outcomes, expected);
    assert_eq!(
        exchange.replies.concat(),
        input_report(&[0xD1, 0xDA, 0x02, 0xFF, 0x00, 0x00])
    );
    assert_eq!(exchange.keyboard.update_mode_options, []);
}

#[test]
fn a_padded_reset_to_update_mode_uses_its_first_seven_bytes_as_stock_does() {
    for length in [9, 64, 128] {
        let mut padded = feature(2, 0);
        padded.resize(length, 0);
        let exchange = set_reports(stock_identity(None), &[(FEATURE_REPORT_1, padded)]);
        assert_eq!(
            exchange.outcomes,
            [Some(Outcome::Accepted)],
            "{length} bytes"
        );
        assert_eq!(exchange.keyboard.update_mode_options, [0], "{length} bytes");
    }
    let mut without_id = feature(2, 0).split_off(1);
    without_id.resize(32, 0xA5);
    let exchange = set_reports(stock_identity(None), &[(FEATURE_REPORT_1, without_id)]);
    assert_eq!(exchange.outcomes, [Some(Outcome::Accepted)]);
    assert_eq!(exchange.keyboard.update_mode_options, [0]);
}

#[test]
fn a_stock_command_arriving_at_a_full_queue_is_stalled_instead_of_dropped() {
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
    let ping = feature(0, 0);
    let request = setup(
        CLASS_INTERFACE_OUT,
        SET_REPORT,
        FEATURE_REPORT_1,
        CHANNEL_INTERFACE,
        8,
    );
    let outcomes: Vec<_> = (0..5)
        .map(|_| host.control_out(run.as_mut(), request, &ping))
        .collect();
    let mut expected = vec![Some(Outcome::Accepted); 4];
    expected.push(Some(Outcome::Rejected));
    assert_eq!(outcomes, expected);
}
