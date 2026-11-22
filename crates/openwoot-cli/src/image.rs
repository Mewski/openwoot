use ihex::Record;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    pub start: Option<u32>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("record {record}: {reason}")]
    Record { record: usize, reason: String },
    #[error("record {record}: address below the first data record")]
    BelowFirstRecord { record: usize },
    #[error("no end-of-file record")]
    NoEndOfFile,
    #[error("no data record")]
    NoData,
    #[error("the file is not ASCII text")]
    NotText,
}

impl Content {
    pub fn parse(file: &[u8]) -> Result<Self, ParseError> {
        if file.trim_ascii_start().starts_with(b":") {
            let text = str::from_utf8(file).map_err(|_| ParseError::NotText)?;
            parse_intel_hex(text)
        } else {
            Ok(Self {
                start: None,
                bytes: file.to_vec(),
            })
        }
    }
}

fn parse_intel_hex(text: &str) -> Result<Content, ParseError> {
    let mut base = 0u32;
    let mut first = None;
    let mut bytes = Vec::new();
    let mut seen_end = false;
    for (index, line) in text.split_whitespace().enumerate() {
        let record = index + 1;
        let parsed = Record::from_record_string(line).map_err(|error| ParseError::Record {
            record,
            reason: error.to_string(),
        })?;
        match parsed {
            Record::Data { offset, value } => {
                let address = base + u32::from(offset);
                let first = *first.get_or_insert(address);
                let position = address
                    .checked_sub(first)
                    .and_then(|position| usize::try_from(position).ok())
                    .ok_or(ParseError::BelowFirstRecord { record })?;
                let end = position + value.len();
                if end > bytes.len() {
                    bytes.resize(end, 0xFF);
                }
                if let Some(target) = bytes.get_mut(position..end) {
                    target.copy_from_slice(&value);
                }
            }
            Record::EndOfFile => seen_end = true,
            Record::ExtendedSegmentAddress(segment) => base = u32::from(segment) << 4,
            Record::ExtendedLinearAddress(upper) => base = u32::from(upper) << 16,
            Record::StartSegmentAddress { .. } | Record::StartLinearAddress(_) => {}
        }
    }
    if !seen_end {
        return Err(ParseError::NoEndOfFile);
    }
    let start = first.ok_or(ParseError::NoData)?;
    Ok(Content {
        start: Some(start),
        bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intel_hex_fills_gaps_and_keeps_the_first_address() {
        let text = ":020000040800F2\n:048000000102030472\n:0280060005066D\n:00000001FF\n";
        let content = Content::parse(text.as_bytes()).unwrap();
        assert_eq!(content.start, Some(0x0800_8000));
        assert_eq!(content.bytes, [1, 2, 3, 4, 0xFF, 0xFF, 5, 6]);
    }

    #[test]
    fn intel_hex_needs_an_end_of_file_record() {
        let text = ":020000040800F2\n:048000000102030472\n";
        assert_eq!(
            Content::parse(text.as_bytes()),
            Err(ParseError::NoEndOfFile)
        );
    }

    #[test]
    fn anything_else_is_a_flat_binary() {
        let content = Content::parse(&[0, 0x80, 0, 0x20]).unwrap();
        assert_eq!(
            content,
            Content {
                start: None,
                bytes: vec![0, 0x80, 0, 0x20]
            }
        );
    }
}
