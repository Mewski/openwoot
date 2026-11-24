use std::{ffi::OsStr, path::Path};

use ihex::{ReaderError, Record};

const MAX_SPAN: u64 = 0x0100_0000;
const BYTE_ORDER_MARK: &[u8] = b"\xEF\xBB\xBF";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    IntelHex,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    pub start: Option<u32>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("record {record} is invalid")]
    InvalidRecord {
        record: usize,
        #[source]
        source: ReaderError,
    },
    #[error("record {record} follows the end-of-file record")]
    AfterEndOfFile { record: usize },
    #[error("record {record} overlaps another data record")]
    OverlappingRecord { record: usize },
    #[error(
        "the data records span {span} bytes, more than the {max} bytes of an image",
        max = MAX_SPAN
    )]
    SpanTooLarge { span: u64 },
    #[error("the end-of-file record is missing")]
    NoEndOfFile,
    #[error("the file holds no data record")]
    NoData,
    #[error("the file is not text")]
    NotText,
}

struct Chunk {
    record: usize,
    address: u32,
    value: Vec<u8>,
}

impl Format {
    #[must_use]
    pub fn detect(path: &Path, file: &[u8]) -> Self {
        let extension = path
            .extension()
            .and_then(OsStr::to_str)
            .map(str::to_ascii_lowercase);
        let named_intel_hex = matches!(extension.as_deref(), Some("hex" | "ihex" | "ihx"));
        let named_binary = extension.as_deref() == Some("bin");
        let looks_like_intel_hex = without_byte_order_mark(file).starts_with(b":");
        if named_intel_hex || (!named_binary && looks_like_intel_hex) {
            Self::IntelHex
        } else {
            Self::Binary
        }
    }
}

impl Content {
    pub fn parse(file: &[u8], format: Format) -> Result<Self, Error> {
        match format {
            Format::IntelHex => {
                let text =
                    str::from_utf8(without_byte_order_mark(file)).map_err(|_| Error::NotText)?;
                parse_intel_hex(text)
            }
            Format::Binary => Ok(Self {
                start: None,
                bytes: file.to_vec(),
            }),
        }
    }
}

fn without_byte_order_mark(file: &[u8]) -> &[u8] {
    file.strip_prefix(BYTE_ORDER_MARK).unwrap_or(file)
}

fn parse_intel_hex(text: &str) -> Result<Content, Error> {
    let mut base = 0u32;
    let mut chunks = Vec::new();
    let mut seen_end = false;
    for (index, line) in text.split_whitespace().enumerate() {
        let record = index + 1;
        if seen_end {
            return Err(Error::AfterEndOfFile { record });
        }
        let parsed = Record::from_record_string(line)
            .map_err(|source| Error::InvalidRecord { record, source })?;
        match parsed {
            Record::Data { offset, value } => chunks.push(Chunk {
                record,
                address: base + u32::from(offset),
                value,
            }),
            Record::EndOfFile => seen_end = true,
            Record::ExtendedSegmentAddress(segment) => base = u32::from(segment) << 4,
            Record::ExtendedLinearAddress(upper) => base = u32::from(upper) << 16,
            Record::StartSegmentAddress { .. } | Record::StartLinearAddress(_) => {}
        }
    }
    if !seen_end {
        return Err(Error::NoEndOfFile);
    }
    chunks.sort_by_key(|chunk| chunk.address);
    let start = chunks
        .first()
        .map(|chunk| chunk.address)
        .ok_or(Error::NoData)?;
    let end = chunks.iter().map(end_of).max().unwrap_or_default();
    let span = end - u64::from(start);
    let len = usize::try_from(span)
        .ok()
        .filter(|_| span <= MAX_SPAN)
        .ok_or(Error::SpanTooLarge { span })?;
    let mut bytes = vec![0xFF; len];
    let mut covered = u64::from(start);
    for chunk in &chunks {
        let address = u64::from(chunk.address);
        if address < covered && !chunk.value.is_empty() {
            return Err(Error::OverlappingRecord {
                record: chunk.record,
            });
        }
        let position = usize::try_from(address - u64::from(start)).unwrap_or(len);
        if let Some(target) = bytes.get_mut(position..position + chunk.value.len()) {
            target.copy_from_slice(&chunk.value);
        }
        covered = covered.max(end_of(chunk));
    }
    Ok(Content {
        start: Some(start),
        bytes,
    })
}

fn end_of(chunk: &Chunk) -> u64 {
    u64::from(chunk.address) + u64::try_from(chunk.value.len()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST_RECORDS: &str = ":020000040800F2\n:048000000102030472\n";

    fn intel_hex(text: &str) -> Result<Content, Error> {
        Content::parse(text.as_bytes(), Format::IntelHex)
    }

    #[test]
    fn intel_hex_fills_gaps_and_keeps_the_first_address() {
        let text = ":020000040800F2\n:048000000102030472\n:0280060005066D\n:00000001FF\n";
        let content = intel_hex(text).unwrap();
        assert_eq!(content.start, Some(0x0800_8000));
        assert_eq!(content.bytes, [1, 2, 3, 4, 0xFF, 0xFF, 5, 6]);
    }

    #[test]
    fn intel_hex_records_may_come_in_any_order() {
        let text = ":020000040800F2\n:0280060005066D\n:048000000102030472\n:00000001FF\n";
        let content = intel_hex(text).unwrap();
        assert_eq!(content.start, Some(0x0800_8000));
        assert_eq!(content.bytes, [1, 2, 3, 4, 0xFF, 0xFF, 5, 6]);
    }

    #[test]
    fn intel_hex_needs_an_end_of_file_record() {
        assert_eq!(intel_hex(FIRST_RECORDS), Err(Error::NoEndOfFile));
    }

    #[test]
    fn a_record_after_the_end_of_file_record_is_refused() {
        let text = format!("{FIRST_RECORDS}:00000001FF\n:0280060005066D\n");
        assert_eq!(intel_hex(&text), Err(Error::AfterEndOfFile { record: 4 }));
    }

    #[test]
    fn overlapping_records_are_refused() {
        let text = format!("{FIRST_RECORDS}:0280020007086D\n:00000001FF\n");
        assert_eq!(
            intel_hex(&text),
            Err(Error::OverlappingRecord { record: 3 })
        );
    }

    #[test]
    fn records_far_apart_are_refused_before_anything_is_allocated() {
        let text = ":020000040000FA\n:0100000001FE\n:02000004FFFFFC\n:01FFF000020E\n:00000001FF\n";
        assert_eq!(
            intel_hex(text),
            Err(Error::SpanTooLarge { span: 0xFFFF_FFF1 })
        );
    }

    #[test]
    fn a_byte_order_mark_before_intel_hex_is_skipped() {
        let text = format!("\u{feff}{FIRST_RECORDS}:00000001FF\n");
        assert_eq!(
            Format::detect(Path::new("image"), text.as_bytes()),
            Format::IntelHex
        );
        assert_eq!(intel_hex(&text).unwrap().bytes, [1, 2, 3, 4]);
    }

    #[test]
    fn the_extension_chooses_the_format() {
        assert_eq!(
            Format::detect(Path::new("image.bin"), FIRST_RECORDS.as_bytes()),
            Format::Binary
        );
        assert_eq!(
            Format::detect(Path::new("image.HEX"), &[0; 4]),
            Format::IntelHex
        );
    }

    #[test]
    fn a_binary_whose_second_byte_is_a_colon_is_a_binary() {
        let file = [0x20, 0x3A, 0x00, 0x20];
        assert_eq!(Format::detect(Path::new("image"), &file), Format::Binary);
        assert_eq!(
            Content::parse(&file, Format::Binary).unwrap(),
            Content {
                start: None,
                bytes: file.to_vec()
            }
        );
    }
}
