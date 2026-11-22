use std::fmt;

use crate::elf::Elf;

const MAX_FILE_LEN: u32 = 4096;
const SOURCE_ROOTS: [&str; 2] = ["/registry/src/", "/rustc/"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Location {
    pub(super) file: String,
    pub(super) line: u32,
    pub(super) column: u32,
}

impl Location {
    pub(super) fn read(elf: &Elf<'_>, address: u32) -> Option<Self> {
        let words = elf.bytes_at(address, 16)?;
        let word = |index: usize| {
            words
                .get(index * 4..index * 4 + 4)
                .and_then(|bytes| bytes.first_chunk())
                .map(|bytes| u32::from_le_bytes(*bytes))
        };
        let (file, len, line, column) = (word(0)?, word(1)?, word(2)?, word(3)?);
        if len == 0 || len > MAX_FILE_LEN || line == 0 {
            return None;
        }
        let name = elf.bytes_at(file, usize::try_from(len).ok()? + 1)?;
        let (terminator, name) = name.split_last()?;
        if *terminator != 0 {
            return None;
        }
        Some(Self {
            file: String::from_utf8(name.to_vec()).ok()?,
            line,
            column,
        })
    }

    pub(super) fn short_file(&self) -> &str {
        SOURCE_ROOTS
            .iter()
            .find_map(|root| {
                let (_, rest) = self.file.split_once(root)?;
                rest.split_once('/').map(|(_, path)| path)
            })
            .unwrap_or(&self.file)
    }
}

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}",
            self.short_file(),
            self.line,
            self.column
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn location(file: &str) -> Location {
        Location {
            file: file.to_owned(),
            line: 54,
            column: 9,
        }
    }

    #[test]
    fn dependency_paths_start_at_the_crate_directory() {
        assert_eq!(
            location("/home/owner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/heapless-0.9.4/src/vec.rs")
                .to_string(),
            "heapless-0.9.4/src/vec.rs:54:9"
        );
    }

    #[test]
    fn standard_library_paths_start_at_the_library() {
        assert_eq!(
            location("/rustc/0ef1bc386/library/core/src/cell.rs").to_string(),
            "library/core/src/cell.rs:54:9"
        );
    }

    #[test]
    fn workspace_paths_stay_relative() {
        assert_eq!(
            location("crates/openwoot/src/scan/shared.rs").to_string(),
            "crates/openwoot/src/scan/shared.rs:54:9"
        );
    }
}
