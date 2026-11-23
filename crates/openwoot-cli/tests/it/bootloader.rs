mod fake;

use std::time::Duration;

use fake::Bootloader;
use futures_lite::future::block_on;
use openwoot_cli::{
    bootloader::{Command, Error, MAX_BLOCKS, Session, Upload},
    transfer,
};

const BLOCK_SIZE: usize = 256;
const BLOCKS: usize = 40;

fn image() -> Vec<u8> {
    (0..=250_u8)
        .cycle()
        .step_by(7)
        .take(BLOCKS * BLOCK_SIZE - 100)
        .collect()
}

fn install(bootloader: &Bootloader) -> (Result<(), Error>, Vec<usize>) {
    install_upload(bootloader, &Upload::new(&image(), BLOCK_SIZE))
}

fn install_upload(bootloader: &Bootloader, upload: &Upload) -> (Result<(), Error>, Vec<usize>) {
    let mut session = Session::new(bootloader.clone());
    let mut progress = Vec::new();
    let result = block_on(session.install(upload, |written| progress.push(written)));
    (result, progress)
}

fn assert_nothing_marked(bootloader: &Bootloader) {
    let state = bootloader.0.borrow();
    assert_eq!(state.marker, u32::MAX);
    assert_eq!(state.rebooted_with, None);
}

#[test]
fn the_fake_computes_the_crc_of_the_bootloader_test_values() {
    assert_eq!(fake::crc24(&[1, 2, 3, 4]), 0x00_0001);
    assert_eq!(fake::crc24(&(0..=255).collect::<Vec<u8>>()), 0xEF_3F62);
}

#[test]
fn state_reports_what_the_read_only_commands_return() {
    let bootloader = Bootloader::new();
    let state = block_on(Session::new(bootloader.clone()).state()).unwrap();
    assert_eq!(
        state.flags.unwrap().to_string(),
        "0x02 (update mode requested by the firmware)"
    );
    assert_eq!(state.version.as_deref(), Some("2.3"));
    assert_eq!(state.serial.as_deref(), Some("A1B2C3D4E5F6"));
    assert_eq!(state.session_crc, Some(0));
    let recorded = bootloader.0.borrow();
    assert_eq!(recorded.commands, [0, 1, 9, 9, 4]);
    assert_eq!(recorded.parameters, [0, 0, 0, 1, 0]);
}

#[test]
fn a_unit_without_a_serial_number_reports_none_and_can_still_be_flashed() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().serial = None;
    let state = block_on(Session::new(bootloader.clone()).state()).unwrap();
    assert_eq!(state.serial, None);
    assert_eq!(state.version.as_deref(), Some("2.3"));
    assert_eq!(bootloader.0.borrow().commands, [0, 1, 9, 9, 4]);
    let (result, _) = install(&bootloader);
    result.unwrap();
}

#[test]
fn install_writes_verifies_and_marks_the_image_complete() {
    let upload = Upload::new(&image(), BLOCK_SIZE);
    let bootloader = Bootloader::new();
    let (result, progress) = install(&bootloader);
    result.unwrap();
    let state = bootloader.0.borrow();
    assert_eq!(state.flash[..upload.bytes.len()], upload.bytes);
    assert_eq!(state.marker, 0);
    assert_eq!(state.rebooted_with, Some(1));
    assert_eq!(progress, (1..=BLOCKS).collect::<Vec<_>>());
    let mut expected = vec![6, 2];
    expected.extend([3; BLOCKS]);
    expected.extend([8, 4, 5]);
    assert_eq!(state.commands, expected);
    let mut parameters = vec![0xFFAA_FFBB, u32::try_from(BLOCKS).unwrap()];
    parameters.extend(0..u32::try_from(BLOCKS).unwrap());
    parameters.extend([0, 0, 1]);
    assert_eq!(state.parameters, parameters);
}

#[test]
fn pings_wait_one_second_and_every_other_reply_four_seconds() {
    let bootloader = Bootloader::new();
    block_on(Session::new(bootloader.clone()).state()).unwrap();
    let (result, _) = install(&bootloader);
    result.unwrap();
    let state = bootloader.0.borrow();
    assert_eq!(state.timeouts.first(), Some(&Duration::from_secs(1)));
    assert!(
        state
            .timeouts
            .iter()
            .skip(1)
            .all(|&timeout| timeout == Duration::from_secs(4))
    );
}

#[test]
fn a_crc_mismatch_stops_before_the_image_is_marked_complete() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.flipped_block = Some(17);
    let (result, _) = install(&bootloader);
    assert!(
        matches!(result, Err(Error::CrcMismatch { reported, expected }) if reported != expected)
    );
    assert_nothing_marked(&bootloader);
    assert_eq!(bootloader.0.borrow().commands.last(), Some(&4));
}

#[test]
fn a_wrong_last_block_stops_before_the_image_is_marked_complete() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.last_block_off_by_one = true;
    let (result, _) = install(&bootloader);
    let last = u32::try_from(BLOCKS).unwrap() - 1;
    assert_eq!(
        result,
        Err(Error::LastBlockMismatch {
            reported: last + 1,
            expected: last
        })
    );
    assert_nothing_marked(&bootloader);
    assert_eq!(bootloader.0.borrow().commands.last(), Some(&8));
}

#[test]
fn a_short_crc_reply_stops_before_the_image_is_marked_complete() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.short_crc = true;
    let (result, _) = install(&bootloader);
    assert_eq!(
        result,
        Err(Error::ShortReply {
            command: Command::Crc,
            len: 2,
            expected: 3
        })
    );
    assert_nothing_marked(&bootloader);
}

#[test]
fn a_wrong_reply_stops_the_upload() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.wrong_echo_to_block = Some(5);
    let (result, progress) = install(&bootloader);
    assert!(matches!(
        result,
        Err(Error::UnexpectedReply {
            command: Command::WriteBlock,
            ..
        })
    ));
    assert_eq!(progress.len(), 5);
    assert_nothing_marked(&bootloader);
}

#[test]
fn a_reply_with_the_wrong_magic_stops_the_upload() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.wrong_magic_to = Some(3);
    let (result, progress) = install(&bootloader);
    assert!(matches!(
        result,
        Err(Error::UnexpectedReply {
            command: Command::WriteBlock,
            ..
        })
    ));
    assert_eq!(progress, [] as [usize; 0]);
    assert_nothing_marked(&bootloader);
}

#[test]
fn a_reply_that_echoes_0xff_is_taken_as_the_answer_to_any_command() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.echo_any_to = Some(3);
    let (result, progress) = install(&bootloader);
    result.unwrap();
    assert_eq!(progress.len(), BLOCKS);
}

#[test]
fn an_empty_reply_counts_as_no_reply() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.empty_reply_to = Some(1);
    assert_eq!(
        block_on(Session::new(bootloader).state()),
        Err(Error::NoReply {
            command: Command::Version,
            timeout: Duration::from_secs(4)
        })
    );
}

#[test]
fn a_late_reply_to_the_previous_request_is_discarded() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.repeated_reply_to = Some(6);
    let (result, _) = install(&bootloader);
    result.unwrap();
    assert_eq!(bootloader.0.borrow().marker, 0);
}

#[test]
fn a_refused_unlock_changes_nothing() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.refused = Some(6);
    let (result, progress) = install(&bootloader);
    assert_eq!(
        result,
        Err(Error::Refused {
            command: Command::Unlock,
            status: 0xFF
        })
    );
    assert_eq!(progress, [] as [usize; 0]);
    let state = bootloader.0.borrow();
    assert_eq!(state.marker, 0);
    assert_eq!(state.commands, [6]);
}

#[test]
fn a_refused_block_stops_the_upload() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.refused = Some(3);
    let (result, progress) = install(&bootloader);
    assert_eq!(
        result,
        Err(Error::Refused {
            command: Command::WriteBlock,
            status: 0xFF
        })
    );
    assert_eq!(progress, [] as [usize; 0]);
    assert_nothing_marked(&bootloader);
}

#[test]
fn a_disappearing_keyboard_stops_the_upload() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.disconnect_after_requests = Some(10);
    let (result, progress) = install(&bootloader);
    assert_eq!(result, Err(Error::Transfer(transfer::Error::Disconnected)));
    assert_eq!(progress.len(), 8);
    assert_nothing_marked(&bootloader);
}

#[test]
fn a_failed_restart_is_reported_after_the_image_was_marked_complete() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.failed_restart = true;
    let (result, _) = install(&bootloader);
    assert!(matches!(result, Err(Error::Restart(_))));
    assert_eq!(bootloader.0.borrow().marker, 0);
}

#[test]
fn ping_is_retried_after_a_pause() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.silent_pings = 2;
    block_on(Session::new(bootloader.clone()).state()).unwrap();
    assert_eq!(bootloader.0.borrow().pauses, 2);
}

#[test]
fn ping_gives_up_after_five_attempts() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.silent_pings = 5;
    assert!(matches!(
        block_on(Session::new(bootloader.clone()).state()),
        Err(Error::NoReply {
            command: Command::Ping,
            ..
        })
    ));
    assert_eq!(bootloader.0.borrow().pauses, 4);
}

#[test]
fn ping_is_not_retried_once_the_keyboard_is_gone() {
    let bootloader = Bootloader::new();
    bootloader.0.borrow_mut().faults.disconnect_after_requests = Some(0);
    assert_eq!(
        block_on(Session::new(bootloader.clone()).state()),
        Err(Error::Transfer(transfer::Error::Disconnected))
    );
    assert_eq!(bootloader.0.borrow().pauses, 0);
}

#[test]
fn an_empty_image_is_not_sent() {
    let bootloader = Bootloader::new();
    let (result, _) = install_upload(&bootloader, &Upload::new(&[], BLOCK_SIZE));
    assert_eq!(result, Err(Error::BlockCountOutOfRange(0)));
    assert_eq!(bootloader.0.borrow().commands, Vec::<u8>::new());
}

#[test]
fn an_image_past_the_last_block_of_the_bootloader_is_not_sent() {
    let bootloader = Bootloader::new();
    let upload = Upload::new(&vec![0; MAX_BLOCKS * BLOCK_SIZE + 1], BLOCK_SIZE);
    let (result, _) = install_upload(&bootloader, &upload);
    assert_eq!(result, Err(Error::BlockCountOutOfRange(MAX_BLOCKS + 1)));
    assert_eq!(bootloader.0.borrow().commands, Vec::<u8>::new());
}

#[test]
fn blocks_of_another_size_are_not_sent() {
    let bootloader = Bootloader::new();
    let (result, _) = install_upload(&bootloader, &Upload::new(&image(), 128));
    assert_eq!(result, Err(Error::BlockSizeUnsupported(128)));
    assert_eq!(bootloader.0.borrow().commands, Vec::<u8>::new());
}
