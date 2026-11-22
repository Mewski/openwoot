use std::{fs, path::PathBuf};

use anyhow::{Context, Result, ensure};
use ihex::Record;

use crate::{
    elf::{Elf, Segment},
    process,
    workspace::{Board, Workspace},
};

const HEX_RECORD_LEN: usize = 16;
const ERASED_BYTE: u8 = 0xFF;

pub(crate) struct Images {
    pub(crate) elf: PathBuf,
    pub(crate) hex: PathBuf,
    pub(crate) bin: PathBuf,
}

pub(crate) fn build(workspace: &Workspace, board: &Board) -> Result<Images> {
    let root = &workspace.root;
    process::execute(process::cargo(root).args([
        "build",
        "--release",
        "--package",
        &board.name,
        "--bin",
        &board.binary,
        "--target",
        &board.target,
    ]))?;
    let linked = workspace
        .target_directory()
        .join(&board.target)
        .join("release")
        .join(&board.binary);
    let directory = root.join("build").join(&board.name);
    fs::create_dir_all(&directory)
        .with_context(|| format!("could not create `{}`", directory.display()))?;
    let images = Images {
        elf: directory.join(format!("{}.elf", board.binary)),
        hex: directory.join(format!("{}.hex", board.binary)),
        bin: directory.join(format!("{}.bin", board.binary)),
    };
    fs::copy(&linked, &images.elf)
        .with_context(|| format!("could not copy `{}`", linked.display()))?;

    let data = fs::read(&images.elf)
        .with_context(|| format!("could not read `{}`", images.elf.display()))?;
    let elf = Elf::parse(&data)?;
    let segments = elf.loadable_segments()?;
    fs::write(&images.bin, flatten(&segments)?)
        .with_context(|| format!("could not write `{}`", images.bin.display()))?;
    fs::write(&images.hex, intel_hex(&segments, elf.entry())?)
        .with_context(|| format!("could not write `{}`", images.hex.display()))?;
    Ok(images)
}

fn flatten(segments: &[Segment<'_>]) -> Result<Vec<u8>> {
    let start = segments
        .first()
        .context("the ELF file has no loadable data")?
        .address;
    let mut bytes = Vec::new();
    for segment in segments {
        let offset = usize::try_from(segment.address - start)?;
        ensure!(
            offset >= bytes.len(),
            "loadable segments overlap at {}",
            openwoot_image::Hex(segment.address)
        );
        bytes.resize(offset, ERASED_BYTE);
        bytes.extend_from_slice(segment.bytes);
    }
    Ok(bytes)
}

fn intel_hex(segments: &[Segment<'_>], entry: u32) -> Result<String> {
    let mut records = Vec::new();
    let mut upper_address = None;
    for segment in segments {
        let mut address = segment.address;
        let mut rest = segment.bytes;
        while !rest.is_empty() {
            let [high, low] = address_halves(address);
            if upper_address != Some(high) {
                records.push(Record::ExtendedLinearAddress(high));
                upper_address = Some(high);
            }
            let to_boundary = 0x1_0000 - usize::from(low);
            let (chunk, tail) = rest.split_at(rest.len().min(HEX_RECORD_LEN).min(to_boundary));
            records.push(Record::Data {
                offset: low,
                value: chunk.to_vec(),
            });
            address += u32::try_from(chunk.len())?;
            rest = tail;
        }
    }
    records.push(Record::StartLinearAddress(entry));
    records.push(Record::EndOfFile);
    Ok(ihex::create_object_file_representation(&records)?)
}

const fn address_halves(address: u32) -> [u16; 2] {
    let [byte0, byte1, byte2, byte3] = address.to_le_bytes();
    [
        u16::from_le_bytes([byte2, byte3]),
        u16::from_le_bytes([byte0, byte1]),
    ]
}
