mod fake;

use fake::Bootloader;
use futures_lite::future::block_on;
use openwoot_cli::{
    bootloader::{Command, Error, Flags, Session, Upload},
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

fn install(bootloader: Bootloader) -> (Result<(), Error>, Bootloader, Vec<usize>) {
    let upload = Upload::new(&image(), BLOCK_SIZE);
    let mut session = Session::new(bootloader);
    let mut progress = Vec::new();
    let result = block_on(session.install(&upload, |written| progress.push(written)));
    (result, session.into_port(), progress)
}

#[test]
fn state_reports_what_the_read_only_commands_return() {
    let mut session = Session::new(Bootloader::new());
    let state = block_on(session.state()).unwrap();
    assert_eq!(state.flags.map(Flags::bits), Some(0x02));
    assert_eq!(
        state.flags.unwrap().to_string(),
        "0x02 (update mode requested by the firmware)"
    );
    assert_eq!(state.version, "2.3");
    assert_eq!(state.serial, "A1B2C3D4E5F6");
    assert_eq!(state.session_crc, 0);
    assert_eq!(session.into_port().commands, [0, 1, 9, 9, 4]);
}

#[test]
fn install_writes_verifies_and_marks_the_image_complete() {
    let upload = Upload::new(&image(), BLOCK_SIZE);
    let (result, bootloader, progress) = install(Bootloader::new());
    result.unwrap();
    assert_eq!(bootloader.flash[..upload.bytes.len()], upload.bytes);
    assert_eq!(bootloader.marker, 0);
    assert_eq!(bootloader.rebooted_with, Some(1));
    assert_eq!(progress, (1..=BLOCKS).collect::<Vec<_>>());
    let mut expected = vec![6, 2];
    expected.extend([3; BLOCKS]);
    expected.extend([8, 4, 5]);
    assert_eq!(bootloader.commands, expected);
}

#[test]
fn a_crc_mismatch_stops_before_the_image_is_marked_complete() {
    let mut bootloader = Bootloader::new();
    bootloader.faults.flipped_block = Some(17);
    let (result, bootloader, _) = install(bootloader);
    assert!(
        matches!(result, Err(Error::CrcMismatch { reported, expected }) if reported != expected)
    );
    assert_eq!(bootloader.marker, u32::MAX);
    assert_eq!(bootloader.rebooted_with, None);
    assert_eq!(bootloader.commands.last(), Some(&4));
}

#[test]
fn a_wrong_reply_stops_the_upload() {
    let mut bootloader = Bootloader::new();
    bootloader.faults.wrong_echo_to_block = Some(5);
    let (result, bootloader, progress) = install(bootloader);
    assert!(matches!(
        result,
        Err(Error::UnexpectedReply {
            command: Command::WriteBlock,
            ..
        })
    ));
    assert_eq!(progress.len(), 5);
    assert_eq!(bootloader.marker, u32::MAX);
    assert_eq!(bootloader.rebooted_with, None);
}

#[test]
fn a_disappearing_keyboard_stops_the_upload() {
    let mut bootloader = Bootloader::new();
    bootloader.faults.disconnect_after_requests = Some(10);
    let (result, bootloader, progress) = install(bootloader);
    assert_eq!(result, Err(Error::Transfer(transfer::Error::Disconnected)));
    assert_eq!(progress.len(), 8);
    assert_eq!(bootloader.marker, u32::MAX);
    assert_eq!(bootloader.rebooted_with, None);
}

#[test]
fn ping_is_retried_after_a_pause() {
    let mut bootloader = Bootloader::new();
    bootloader.faults.silent_pings = 2;
    let mut session = Session::new(bootloader);
    block_on(session.state()).unwrap();
    assert_eq!(session.into_port().pauses, 2);
}

#[test]
fn ping_gives_up_after_five_attempts() {
    let mut bootloader = Bootloader::new();
    bootloader.faults.silent_pings = 5;
    let mut session = Session::new(bootloader);
    assert!(matches!(
        block_on(session.state()),
        Err(Error::NoReply {
            command: Command::Ping,
            ..
        })
    ));
    assert_eq!(session.into_port().pauses, 4);
}

#[test]
fn a_refused_request_reports_its_status() {
    let mut bootloader = Bootloader::new();
    bootloader.serial = None;
    let mut session = Session::new(bootloader);
    assert_eq!(
        block_on(session.state()),
        Err(Error::Refused {
            command: Command::Serial,
            status: 0xFF
        })
    );
}

#[test]
fn an_empty_image_is_not_sent() {
    let mut session = Session::new(Bootloader::new());
    let upload = Upload::new(&[], BLOCK_SIZE);
    assert_eq!(
        block_on(session.install(&upload, |_| {})),
        Err(Error::BlockCountOutOfRange(0))
    );
    assert_eq!(session.into_port().commands, Vec::<u8>::new());
}
