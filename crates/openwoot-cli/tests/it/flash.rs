use std::{error::Error, fs, path::PathBuf};

use ihex::Record;
use openwoot_image::{Descriptor, Region, UpdateMode, UpdateProtocol, UsbIds};
use predicates::str::contains;

use crate::support::openwoot;

const START: u32 = 0x0800_8000;
const AREA_LEN: u32 = 0x1_0000;
const RAM: Region = Region {
    start: 0x2000_0000,
    end: 0x2003_0000,
};
const DESCRIPTOR_OFFSET: usize = 0x1C0;
const UPDATE_PROTOCOL_OFFSET: usize = DESCRIPTOR_OFFSET + 32;
const IMAGE_LEN: usize = 0x300;
const RESET_OFFSET: u32 = 0x209;
const GOOD_RESET: u32 = START + RESET_OFFSET;
const UPDATE_MODE: UpdateMode = UpdateMode {
    protocol: UpdateProtocol::WootingFactoryBootloader,
    product_id: 0x567F,
    block_size: 256,
};

fn write_image(
    name: &str,
    reset: u32,
    update_mode: Option<UpdateMode>,
) -> Result<PathBuf, Box<dyn Error>> {
    write_image_at(name, START, reset, update_mode)
}

fn write_image_at(
    name: &str,
    start: u32,
    reset: u32,
    update_mode: Option<UpdateMode>,
) -> Result<PathBuf, Box<dyn Error>> {
    write_hex(
        name,
        start,
        &image_bytes(start, RAM.end, reset, update_mode),
    )
}

fn image_bytes(
    start: u32,
    stack_pointer: u32,
    reset: u32,
    update_mode: Option<UpdateMode>,
) -> Vec<u8> {
    let descriptor = Descriptor::new(
        "Test board",
        UsbIds {
            vendor_id: 0x1234,
            product_id_base: 0x5670,
        },
        Region {
            start,
            end: start + AREA_LEN,
        },
        RAM,
        update_mode,
    );
    let mut bytes = Vec::new();
    bytes.extend(stack_pointer.to_le_bytes());
    bytes.extend(reset.to_le_bytes());
    bytes.resize(DESCRIPTOR_OFFSET, 0);
    bytes.extend(descriptor.to_bytes());
    bytes.resize(IMAGE_LEN, 0);
    bytes
}

fn write_hex(name: &str, start: u32, bytes: &[u8]) -> Result<PathBuf, Box<dyn Error>> {
    let first_offset = u16::try_from(start & 0xFFFF)?;
    let mut records = vec![Record::ExtendedLinearAddress(u16::try_from(start >> 16)?)];
    records.extend(
        bytes
            .chunks(16)
            .zip((first_offset..).step_by(16))
            .map(|(chunk, offset)| Record::Data {
                offset,
                value: chunk.to_vec(),
            }),
    );
    records.push(Record::EndOfFile);
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    fs::write(&path, ihex::create_object_file_representation(&records)?)?;
    Ok(path)
}

#[test]
fn dry_run_accepts_a_good_image() -> Result<(), Box<dyn Error>> {
    let path = write_image("good.hex", GOOD_RESET, Some(UPDATE_MODE))?;
    openwoot()
        .args(["flash", "--dry-run"])
        .arg(&path)
        .assert()
        .success()
        .stdout(contains("board:        Test board"))
        .stdout(contains("ram:          0x2000_0000-0x2003_0000"))
        .stdout(contains("usb ids:      1234:5670"))
        .stdout(contains(
            "update mode:  1234:567f, Wooting factory bootloader",
        ))
        .stdout(contains("upload:       3 blocks of 256 bytes, CRC-24 0x00"))
        .stdout(contains("dry run:      nothing was sent"));
    Ok(())
}

#[test]
fn dry_run_refuses_a_reset_vector_outside_the_image() -> Result<(), Box<dyn Error>> {
    let path = write_image("bad-reset.hex", START + 0x8001, Some(UPDATE_MODE))?;
    openwoot()
        .args(["flash", "--dry-run"])
        .arg(&path)
        .assert()
        .failure()
        .stderr(contains("openwoot: `"))
        .stderr(contains(
            "is not an openwoot firmware image: reset vector 0x0801_0001 does not point",
        ));
    Ok(())
}

#[test]
fn dry_run_refuses_a_stack_pointer_that_is_not_the_end_of_ram() -> Result<(), Box<dyn Error>> {
    let bytes = image_bytes(START, RAM.end - 0x100, GOOD_RESET, Some(UPDATE_MODE));
    let path = write_hex("low-stack.hex", START, &bytes)?;
    openwoot()
        .args(["flash", "--dry-run"])
        .arg(&path)
        .assert()
        .failure()
        .stdout("")
        .stderr(contains(
            "is not an openwoot firmware image: initial stack pointer 0x2002_ff00 is not the end \
             of RAM at 0x2003_0000",
        ));
    Ok(())
}

#[test]
fn dry_run_refuses_an_update_protocol_this_tool_does_not_implement() -> Result<(), Box<dyn Error>> {
    let mut bytes = image_bytes(START, RAM.end, GOOD_RESET, Some(UPDATE_MODE));
    bytes[UPDATE_PROTOCOL_OFFSET..UPDATE_PROTOCOL_OFFSET + 4].copy_from_slice(&2_u32.to_le_bytes());
    let path = write_hex("unknown-protocol.hex", START, &bytes)?;
    openwoot()
        .args(["flash", "--dry-run"])
        .arg(&path)
        .assert()
        .failure()
        .stdout("")
        .stderr(contains(
            "is not an openwoot firmware image: the image descriptor is invalid: update protocol 2 \
             is unknown",
        ));
    Ok(())
}

#[test]
fn flashing_an_update_protocol_this_tool_does_not_implement_is_refused_before_any_device_access()
-> Result<(), Box<dyn Error>> {
    let mut bytes = image_bytes(START, RAM.end, GOOD_RESET, Some(UPDATE_MODE));
    bytes[UPDATE_PROTOCOL_OFFSET..UPDATE_PROTOCOL_OFFSET + 4]
        .copy_from_slice(&0x100_u32.to_le_bytes());
    let path = write_hex("unknown-protocol-flash.hex", START, &bytes)?;
    openwoot()
        .arg("flash")
        .arg(&path)
        .assert()
        .failure()
        .stdout("")
        .stderr(contains("update protocol 256 is unknown"));
    Ok(())
}

#[test]
fn dry_run_refuses_a_file_that_is_not_intel_hex() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("broken.hex");
    fs::write(&path, ":020000040800F2\n:0480000001020304FF\n:00000001FF\n")?;
    openwoot()
        .args(["flash", "--dry-run"])
        .arg(&path)
        .assert()
        .failure()
        .stderr(contains("as Intel HEX: record 2"));
    Ok(())
}

#[test]
fn dry_run_shows_a_board_without_update_mode() -> Result<(), Box<dyn Error>> {
    let path = write_image("no-update.hex", GOOD_RESET, None)?;
    openwoot()
        .args(["flash", "--dry-run"])
        .arg(&path)
        .assert()
        .success()
        .stdout(contains("update mode:  not supported by this board"));
    Ok(())
}

#[test]
fn dry_run_refuses_blocks_the_bootloader_does_not_take() -> Result<(), Box<dyn Error>> {
    let update_mode = UpdateMode {
        block_size: 512,
        ..UPDATE_MODE
    };
    let path = write_image("large-blocks.hex", GOOD_RESET, Some(update_mode))?;
    openwoot()
        .args(["flash", "--dry-run"])
        .arg(&path)
        .assert()
        .failure()
        .stderr(contains(
            "openwoot: the image cannot be written through the factory bootloader; nothing was \
             sent: the image names blocks of 512 bytes; the bootloader takes blocks of 256 bytes",
        ));
    Ok(())
}

#[test]
fn dry_run_refuses_an_image_area_where_the_bootloader_does_not_write() -> Result<(), Box<dyn Error>>
{
    let start = 0x0800_0000;
    let path = write_image_at("low.hex", start, start + RESET_OFFSET, Some(UPDATE_MODE))?;
    openwoot()
        .args(["flash", "--dry-run"])
        .arg(&path)
        .assert()
        .failure()
        .stderr(contains(
            "the image area starts at 0x0800_0000; the bootloader writes the first block to \
             0x0800_8000",
        ));
    Ok(())
}

#[test]
fn a_binary_whose_name_says_so_is_not_read_as_intel_hex() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("colon.bin");
    fs::write(&path, [0x20, 0x3A, 0x00, 0x20])?;
    openwoot()
        .args(["flash", "--dry-run"])
        .arg(&path)
        .assert()
        .failure()
        .stderr(contains("is not an openwoot firmware image"));
    Ok(())
}

#[test]
fn flashing_a_board_without_update_mode_is_refused_before_any_device_access()
-> Result<(), Box<dyn Error>> {
    let path = write_image("refused.hex", GOOD_RESET, None)?;
    openwoot()
        .arg("flash")
        .arg(&path)
        .assert()
        .failure()
        .stderr(contains(
            "openwoot: the board `Test board` has no known update mode; nothing was sent",
        ));
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn a_full_stdout_ends_the_command_with_an_error_instead_of_an_abort() -> Result<(), Box<dyn Error>>
{
    let path = write_image("full.hex", GOOD_RESET, Some(UPDATE_MODE))?;
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_openwoot"))
        .args(["flash", "--dry-run"])
        .arg(&path)
        .stdout(fs::File::create("/dev/full")?)
        .output()?;
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("openwoot: could not write to stdout")
    );
    Ok(())
}

#[test]
fn a_missing_file_is_reported() {
    openwoot()
        .args(["flash", "--dry-run", "missing.hex"])
        .assert()
        .failure()
        .stderr(contains("openwoot: could not read `missing.hex`: "));
}
