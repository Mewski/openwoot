use std::{error::Error, fs, path::PathBuf};

use assert_cmd::Command;
use ihex::Record;
use openwoot_image::{Descriptor, Region, UpdateMode, UsbIds};

const START: u32 = 0x0800_8000;
const STACK_TOP: u32 = 0x2003_0000;
const DESCRIPTOR_OFFSET: usize = 0x1C0;
const IMAGE_LEN: usize = 0x300;

fn write_image(name: &str, reset: u32) -> Result<PathBuf, Box<dyn Error>> {
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
        Some(UpdateMode {
            product_id: 0x567F,
            block_size: 256,
        }),
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

fn openwoot() -> Command {
    Command::new(env!("CARGO_BIN_EXE_openwoot"))
}

#[test]
fn dry_run_accepts_a_good_image() -> Result<(), Box<dyn Error>> {
    let path = write_image("good.hex", START + 0x201)?;
    openwoot()
        .args(["flash", "--dry-run"])
        .arg(&path)
        .assert()
        .success()
        .stdout(predicates::str::contains("Test board"))
        .stdout(predicates::str::contains("3 blocks of 256 bytes"));
    Ok(())
}

#[test]
fn dry_run_refuses_a_reset_vector_outside_the_image() -> Result<(), Box<dyn Error>> {
    let path = write_image("bad-reset.hex", START + 0x8001)?;
    openwoot()
        .args(["flash", "--dry-run"])
        .arg(&path)
        .assert()
        .failure()
        .stderr(predicates::str::contains("reset vector 0x08010001"));
    Ok(())
}

#[test]
fn flashing_without_dry_run_is_refused() -> Result<(), Box<dyn Error>> {
    let path = write_image("refused.hex", START + 0x201)?;
    openwoot()
        .arg("flash")
        .arg(&path)
        .assert()
        .failure()
        .stderr(predicates::str::contains("not implemented yet"));
    Ok(())
}
