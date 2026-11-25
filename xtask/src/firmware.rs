use std::{fs, path::PathBuf};

use anyhow::{Context, Result, ensure};
use cargo_metadata::Message;
use ihex::Record;

use crate::{
    elf::{Elf, Segment},
    process,
    workspace::{Board, Workspace},
};

const HEX_RECORD_LEN: usize = 16;
const ERASED_BYTE: u8 = 0xFF;
const LINK_MAP: &str = "link.map";

pub(crate) struct Images {
    pub(crate) elf: PathBuf,
    pub(crate) hex: PathBuf,
    pub(crate) bin: PathBuf,
    pub(crate) map: PathBuf,
}

pub(crate) fn build(workspace: &Workspace, board: &Board) -> Result<Images> {
    let root = &workspace.root;
    let messages = process::messages(process::cargo(root).args([
        "build",
        "--release",
        "--package",
        &board.firmware,
        "--bin",
        &board.binary,
        "--target",
        &board.target,
    ]))?;
    let (package, linked) = messages
        .iter()
        .find_map(|message| match message {
            Message::CompilerArtifact(artifact) if artifact.target.name == board.binary => {
                Some((&artifact.package_id, artifact.executable.as_ref()?))
            }
            _ => None,
        })
        .with_context(|| format!("cargo built no executable `{}`", board.binary))?;
    let link_map = messages
        .iter()
        .find_map(|message| match message {
            Message::BuildScriptExecuted(script) if script.package_id == *package => {
                Some(script.out_dir.join(LINK_MAP))
            }
            _ => None,
        })
        .with_context(|| format!("the build script of `{}` did not run", board.firmware))?;
    let directory = root.join("build").join(&board.name);
    fs::create_dir_all(&directory)
        .with_context(|| format!("could not create `{}`", directory.display()))?;
    let images = Images {
        elf: directory.join(format!("{}.elf", board.binary)),
        hex: directory.join(format!("{}.hex", board.binary)),
        bin: directory.join(format!("{}.bin", board.binary)),
        map: directory.join(format!("{}.map", board.binary)),
    };
    fs::copy(linked, &images.elf).with_context(|| format!("could not copy `{linked}`"))?;
    fs::copy(&link_map, &images.map)
        .with_context(|| format!("could not copy `{link_map}`; boards link with `-Map`"))?;

    let file = fs::read(&images.elf)
        .with_context(|| format!("could not read `{}`", images.elf.display()))?;
    let elf = Elf::parse(&file)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_are_joined_with_erased_bytes_between_them() {
        let segments = [
            Segment {
                address: 0x0800_8000,
                bytes: &[1, 2],
            },
            Segment {
                address: 0x0800_8004,
                bytes: &[3],
            },
        ];
        assert_eq!(flatten(&segments).unwrap(), [1, 2, 0xFF, 0xFF, 3]);
    }

    #[test]
    fn overlapping_segments_are_rejected() {
        let segments = [
            Segment {
                address: 0x0800_8000,
                bytes: &[1, 2, 3],
            },
            Segment {
                address: 0x0800_8002,
                bytes: &[4],
            },
        ];
        assert_eq!(
            flatten(&segments).unwrap_err().to_string(),
            "loadable segments overlap at 0x0800_8002"
        );
        assert!(flatten(&[]).is_err());
    }

    #[test]
    fn hex_records_end_at_sixteen_bytes_and_at_each_64_kib_boundary() {
        let bytes = [0xAB_u8; 20];
        let segments = [Segment {
            address: 0x0800_FFF8,
            bytes: &bytes,
        }];
        let hex = intel_hex(&segments, 0x0800_8001)
            .unwrap()
            .to_ascii_uppercase();
        let records: Vec<&str> = hex.lines().collect();
        assert_eq!(records.len(), 6);
        assert_eq!(records[0], ":020000040800F2");
        assert!(records[1].starts_with(":08FFF800ABABABAB"));
        assert_eq!(records[2], ":020000040801F1");
        assert!(records[3].starts_with(":0C000000ABABABAB"));
        assert_eq!(records[4], ":04000005080080016E");
        assert_eq!(records[5], ":00000001FF");
    }
}
