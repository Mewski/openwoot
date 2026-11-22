mod fake;

use fake::Keyboard;
use futures_lite::future::block_on;
use openwoot_cli::{
    channel,
    keyboard::{Error, Firmware, Running},
};
use openwoot_protocol::{
    FirmwareVersion, PROTOCOL_VERSION, Version, WireError,
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
    let keyboard = Keyboard::openwoot(OPENWOOT, true);
    let running = block_on(Running::identify(keyboard.clone())).unwrap();
    assert_eq!(running.firmware(), &Firmware::Openwoot(OPENWOOT));
    assert_eq!(running.firmware().to_string(), "openwoot firmware 0.1.0");
    block_on(running.enter_update_mode()).unwrap();
    let state = keyboard.0.borrow();
    assert_eq!(state.update_mode_requests, 1);
    assert_eq!(state.feature_commands, []);
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
fn openwoot_without_the_update_mode_request_points_to_the_key_combination() {
    let keyboard = Keyboard::openwoot(OPENWOOT, false);
    let running = block_on(Running::identify(keyboard.clone())).unwrap();
    let error = block_on(running.enter_update_mode()).unwrap_err();
    assert!(matches!(&error, Error::UpdateModeNotSupported(version) if version == "0.1.0"));
    assert!(error.to_string().contains("recovery key combination"));
    assert_eq!(keyboard.0.borrow().feature_commands, []);
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
    let mut keyboard = Keyboard::openwoot(OPENWOOT, true);
    keyboard.0.borrow_mut().diagnostics = Some(report);
    assert_eq!(
        block_on(channel::diagnostics(&mut keyboard)).unwrap(),
        Some(report)
    );
}

#[test]
fn firmware_without_diagnostics_answers_the_request_as_unknown() {
    let mut keyboard = Keyboard::openwoot(OPENWOOT, true);
    assert!(matches!(
        block_on(channel::diagnostics(&mut keyboard)),
        Err(channel::Error::Reply(openwoot_protocol::Error::Device(
            WireError::UnknownKey
        )))
    ));
}
