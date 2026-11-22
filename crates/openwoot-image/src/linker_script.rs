use crate::Region;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryLayout {
    pub flash: Region,
    pub ram: Region,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum LinkerScriptError {
    #[error("`memory.x` has no `{0}` region")]
    MissingRegion(&'static str),
    #[error("the `{field}` of region `{region}` in `memory.x` is missing or not a number")]
    InvalidField {
        region: &'static str,
        field: &'static str,
    },
    #[error("region `{0}` in `memory.x` is empty or ends beyond the address space")]
    InvalidBounds(&'static str),
}

impl MemoryLayout {
    pub fn parse(memory_x: &str) -> Result<Self, LinkerScriptError> {
        Ok(Self {
            flash: region(memory_x, "FLASH")?,
            ram: region(memory_x, "RAM")?,
        })
    }
}

fn region(memory_x: &str, name: &'static str) -> Result<Region, LinkerScriptError> {
    let mut tokens = memory_x
        .lines()
        .map(|line| {
            line.split(|character: char| {
                character.is_whitespace() || matches!(character, ':' | ',' | '=' | '(')
            })
            .filter(|token| !token.is_empty())
        })
        .find(|tokens| tokens.clone().next() == Some(name))
        .ok_or(LinkerScriptError::MissingRegion(name))?;
    let mut field = |key: &'static str| {
        tokens
            .find(|token| *token == key)
            .and_then(|_| tokens.next())
            .and_then(number)
            .ok_or(LinkerScriptError::InvalidField {
                region: name,
                field: key,
            })
    };
    let start = field("ORIGIN")?;
    let length = field("LENGTH")?;
    match start.checked_add(length) {
        Some(end) if length > 0 => Ok(Region { start, end }),
        _ => Err(LinkerScriptError::InvalidBounds(name)),
    }
}

fn number(token: &str) -> Option<u32> {
    match token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
    {
        Some(digits) => u32::from_str_radix(digits, 16).ok(),
        None => token.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMORY_X: &str = "MEMORY\n{\n  FLASH (rx) : ORIGIN = 0x08007000, LENGTH = 0x000F6000\n  \
                            RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 0x00018000\n}\n";

    #[test]
    fn parses_both_regions() {
        assert_eq!(
            MemoryLayout::parse(MEMORY_X),
            Ok(MemoryLayout {
                flash: Region {
                    start: 0x0800_7000,
                    end: 0x080F_D000
                },
                ram: Region {
                    start: 0x2000_0000,
                    end: 0x2001_8000
                },
            })
        );
    }

    #[test]
    fn reports_a_missing_region() {
        let text = MEMORY_X.replace("RAM", "SRAM");
        assert_eq!(
            MemoryLayout::parse(&text),
            Err(LinkerScriptError::MissingRegion("RAM"))
        );
    }
}
