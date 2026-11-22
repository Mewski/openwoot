use std::{error::Error, fs, path::PathBuf};

use ihex::Record;
use openwoot_image::{Descriptor, Region, UpdateMode, UsbIds};
use predicates::str::contains;

use crate::support::openwoot;

const START: u32 = 0x0800_8000;
const STACK_TOP: u32 = 0x2003_0000;
const DESCRIPTOR_OFFSET: usize = 0x1C0;
const IMAGE_LEN: usize = 0x300;
const GOOD_RESET: u32 = START + 0x201;
const UPDATE_MODE: UpdateMode = UpdateMode {
    product_id: 0x567F,
    block_size: 256,
};

fn write_image(
    name: &str,
    reset: u32,
    update_mode: Option<UpdateMode>,
) -> Result<PathBuf, Box<dyn Error>> {
    let descriptor = Descriptor::new(
        "Test board",
        UsbIds {
            vendor_id: 0x1234,
            product_id_base: 0x5670,
        },
        Region {
            start: START,
            end: START + 0x1_0000,
        },
        update_mode,
    );
    let mut bytes = Vec::new();
    bytes.extend(STACK_TOP.to_le_bytes());
    bytes.extend(reset.to_le_bytes());
    bytes.resize(DESCRIPTOR_OFFSET, 0);
    bytes.extend(descriptor.to_bytes());
    bytes.resize(IMAGE_LEN, 0);

    let mut records = vec![Record::ExtendedLinearAddress(0x0800)];
    records.extend(
        bytes
            .chunks(16)
            .zip((0x8000_u16..).step_by(16))
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
        .stdout(contains("usb ids:      1234:5670"))
        .stdout(contains(
            "update mode:  1234:567f, 3 blocks of 256 bytes, CRC-24 0x00",
        ))
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
fn flashing_a_board_without_update_mode_is_refused_before_any_device_access()
-> Result<(), Box<dyn Error>> {
    let path = write_image("refused.hex", GOOD_RESET, None)?;
    openwoot()
        .arg("flash")
        .arg(&path)
        .assert()
        .failure()
        .stderr(contains(
            "openwoot: the Test board has no known update mode; nothing was sent",
        ));
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
