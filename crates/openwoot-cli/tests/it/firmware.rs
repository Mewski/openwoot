mod fake;

use fake::{Keyboard, Reaction};
use futures_lite::future::block_on;
use openwoot_cli::{
    channel,
    keyboard::{self, Error, Firmware, Running},
    stock,
};
use openwoot_protocol::{
    FirmwareVersion, PROTOCOL_VERSION, Version,
    diagnostics::{Keys, Report, Scan},
};

const OPENWOOT: Version = Version {
    protocol: PROTOCOL_VERSION,
    firmware: FirmwareVersion {
        major: 0,
        minor: 1,
        patch: 0,
    },
};

#[test]
fn openwoot_is_asked_over_the_configuration_channel_although_it_answers_stock_commands() {
    let keyboard = Keyboard::openwoot(OPENWOOT);
    let running = block_on(Running::identify(keyboard.clone())).unwrap();
    assert_eq!(running.firmware(), &Firmware::Openwoot(OPENWOOT));
    assert_eq!(running.firmware().to_string(), "openwoot firmware 0.1.0");
    block_on(running.enter_update_mode()).unwrap();
    let state = keyboard.0.borrow();
    assert_eq!(state.update_mode_requests, 1);
    assert_eq!(state.feature_commands, []);
}

#[test]
fn openwoot_that_resets_before_it_answers_has_entered_update_mode() {
    let keyboard = Keyboard::openwoot(OPENWOOT);
    keyboard.0.borrow_mut().on_update_mode_request = Reaction::Disconnect;
    let running = block_on(Running::identify(keyboard.clone())).unwrap();
    block_on(running.enter_update_mode()).unwrap();
    let state = keyboard.0.borrow();
    assert_eq!(state.update_mode_requests, 1);
    assert_eq!(state.feature_commands, []);
}

#[test]
fn openwoot_without_the_update_mode_request_is_reset_with_the_stock_command() {
    let keyboard = Keyboard::openwoot(OPENWOOT);
    keyboard.0.borrow_mut().on_update_mode_request = Reaction::Refuse;
    let running = block_on(Running::identify(keyboard.clone())).unwrap();
    block_on(running.enter_update_mode()).unwrap();
    let state = keyboard.0.borrow();
    assert_eq!(state.update_mode_requests, 0);
    assert_eq!(state.feature_commands, [(2, 0)]);
}

#[test]
fn openwoot_of_another_protocol_is_identified_and_reset_with_stock_commands() {
    let keyboard = Keyboard::openwoot(OPENWOOT);
    keyboard.0.borrow_mut().foreign_protocol = true;
    let running = block_on(Running::identify(keyboard.clone())).unwrap();
    assert_eq!(
        running.firmware(),
        &Firmware::OtherProtocol("2.1.2".to_owned())
    );
    block_on(running.enter_update_mode()).unwrap();
    let state = keyboard.0.borrow();
    assert_eq!(state.update_mode_requests, 0);
    assert_eq!(state.feature_commands, [(1, 0), (2, 0)]);
}

#[test]
fn stock_firmware_is_asked_with_its_reset_command() {
    let keyboard = Keyboard::stock([2, 14, 1]);
    let running = block_on(Running::identify(keyboard.clone())).unwrap();
    assert_eq!(running.firmware(), &Firmware::Stock("2.14.1".to_owned()));
    assert_eq!(running.firmware().to_string(), "stock firmware 2.14.1");
    block_on(running.enter_update_mode()).unwrap();
    assert_eq!(keyboard.0.borrow().feature_commands, [(1, 0), (2, 0)]);
}

#[test]
fn a_stock_answer_with_an_error_status_is_not_taken_as_a_version() {
    let keyboard = Keyboard::stock([2, 14, 1]);
    keyboard.0.borrow_mut().busy_stock_version = true;
    assert!(matches!(
        block_on(Running::identify(keyboard.clone())),
        Err(Error::Stock(stock::Error::UnexpectedReply {
            command: 1,
            ..
        }))
    ));
    assert_eq!(keyboard.0.borrow().feature_commands, [(1, 0)]);
}

#[test]
fn a_keyboard_that_answers_nothing_is_left_alone() {
    let keyboard = Keyboard::default();
    assert!(matches!(
        block_on(Running::identify(keyboard.clone())),
        Err(Error::NoVersionAnswer)
    ));
    assert_eq!(keyboard.0.borrow().feature_commands, [(1, 0)]);
}

#[test]
fn every_request_carries_its_own_sequence_number() {
    let keyboard = Keyboard::openwoot(OPENWOOT);
    block_on(Running::identify(keyboard.clone())).unwrap();
    block_on(Running::identify(keyboard.clone())).unwrap();
    let state = keyboard.0.borrow();
    assert_eq!(state.sequence_numbers.len(), 2);
    assert_ne!(
        state.sequence_numbers.first(),
        state.sequence_numbers.last()
    );
}

#[test]
fn a_late_answer_to_an_earlier_request_is_not_taken_for_the_current_one() {
    let keyboard = Keyboard::openwoot(OPENWOOT);
    block_on(Running::identify(keyboard.clone())).unwrap();
    keyboard.0.borrow_mut().stale_version = Some(Version {
        protocol: PROTOCOL_VERSION,
        firmware: FirmwareVersion {
            major: 9,
            minor: 9,
            patch: 9,
        },
    });
    let running = block_on(Running::identify(keyboard.clone())).unwrap();
    assert_eq!(running.firmware(), &Firmware::Openwoot(OPENWOOT));
}

#[test]
fn diagnostics_come_back_over_the_configuration_channel() {
    let report = Report {
        scan: Scan {
            cycles: 7,
            recoveries: 0,
            overruns: None,
        },
        keys: Keys {
            passes: 7,
            reports: 1,
        },
        ..Report::default()
    };
    let mut device = Keyboard::openwoot(OPENWOOT);
    device.0.borrow_mut().diagnostics = Some(report);
    assert_eq!(
        block_on(channel::diagnostics(&mut device)).unwrap(),
        Some(report)
    );
    assert_eq!(
        block_on(keyboard::read_diagnostics(&mut device)).unwrap(),
        report
    );
}

#[test]
fn diagnostics_refused_by_firmware_of_another_protocol_name_both_versions() {
    let mut device = Keyboard::openwoot(Version {
        protocol: PROTOCOL_VERSION + 1,
        ..OPENWOOT
    });
    assert!(matches!(
        block_on(keyboard::read_diagnostics(&mut device)),
        Err(Error::OtherProtocol { firmware, tool })
            if firmware == PROTOCOL_VERSION + 1 && tool == PROTOCOL_VERSION
    ));
}

#[test]
fn diagnostics_refused_by_firmware_of_this_protocol_keep_the_answer() {
    let mut device = Keyboard::openwoot(OPENWOOT);
    assert!(matches!(
        block_on(keyboard::read_diagnostics(&mut device)),
        Err(Error::Channel(channel::Error::Reply(_)))
    ));
}
