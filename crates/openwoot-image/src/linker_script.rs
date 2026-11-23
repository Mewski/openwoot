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
    #[error("`memory.x` defines the `{0}` region more than once")]
    DuplicateRegion(&'static str),
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
    let mut definitions = uncommented_lines(memory_x)
        .map(|line| {
            line.split(|character: char| {
                character.is_whitespace() || matches!(character, ':' | ',' | '=' | '(')
            })
            .filter(|token| !token.is_empty())
        })
        .filter(|tokens| tokens.clone().next() == Some(name));
    let mut tokens = definitions
        .next()
        .ok_or(LinkerScriptError::MissingRegion(name))?;
    if definitions.next().is_some() {
        return Err(LinkerScriptError::DuplicateRegion(name));
    }
    let start = tokens
        .find(|token| *token == "ORIGIN")
        .and_then(|_| tokens.next())
        .and_then(number)
        .ok_or(LinkerScriptError::InvalidField {
            region: name,
            field: "ORIGIN",
        })?;
    let length = tokens
        .next()
        .filter(|token| *token == "LENGTH")
        .and_then(|_| tokens.next())
        .and_then(number)
        .filter(|_| tokens.next().is_none())
        .ok_or(LinkerScriptError::InvalidField {
            region: name,
            field: "LENGTH",
        })?;
    match start.checked_add(length) {
        Some(end) if length > 0 => Ok(Region { start, end }),
        _ => Err(LinkerScriptError::InvalidBounds(name)),
    }
}

fn uncommented_lines(memory_x: &str) -> impl Iterator<Item = &str> {
    let mut commented = false;
    memory_x.lines().filter(move |line| {
        let inside = commented;
        commented = match (line.rfind("/*"), line.rfind("*/")) {
            (Some(opened), Some(closed)) => opened > closed,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => commented,
        };
        !inside
    })
}

fn number(token: &str) -> Option<u32> {
    match token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
    {
        Some(digits) if digits.bytes().all(|digit| digit.is_ascii_hexdigit()) => {
            u32::from_str_radix(digits, 16).ok()
        }
        None if token.bytes().all(|digit| digit.is_ascii_digit()) => token.parse().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MEMORY_X: &str = "MEMORY\n{\n  FLASH (rx) : ORIGIN = 0x08007000, LENGTH = 0x000F6000\n  \
                            RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 0x00018000\n}\n";

    fn flash(definition: &str) -> Result<Region, LinkerScriptError> {
        let memory_x = format!(
            "MEMORY\n{{\n  {definition}\n  RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 0x00018000\n}}\n"
        );
        MemoryLayout::parse(&memory_x).map(|layout| layout.flash)
    }

    fn ram(definition: &str) -> Result<Region, LinkerScriptError> {
        let memory_x = format!(
            "MEMORY\n{{\n  FLASH (rx) : ORIGIN = 0x08008000, LENGTH = 0x000F6000\n  {definition}\n}}\n"
        );
        MemoryLayout::parse(&memory_x).map(|layout| layout.ram)
    }

    fn invalid(field: &'static str) -> Result<Region, LinkerScriptError> {
        Err(LinkerScriptError::InvalidField {
            region: "FLASH",
            field,
        })
    }

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

    #[test]
    fn reads_hexadecimal_and_decimal_numbers_without_spaces_too() {
        let expected = Ok(Region {
            start: 0x0800_8000,
            end: 0x0800_8100,
        });
        assert_eq!(
            flash("FLASH (rx) : ORIGIN = 0X08008000, LENGTH = 256"),
            expected
        );
        assert_eq!(flash("FLASH(rx):ORIGIN=0x08008000,LENGTH=0x100"), expected);
    }

    #[test]
    fn refuses_an_expression_instead_of_reading_its_first_number() {
        assert_eq!(
            flash("FLASH (rx) : ORIGIN = 0x08008000, LENGTH = 0x100000 - 0x8000 - 0x2000"),
            invalid("LENGTH")
        );
        assert_eq!(
            flash("FLASH (rx) : ORIGIN = 0x08000000 + 0x8000, LENGTH = 0xF6000"),
            invalid("LENGTH")
        );
        assert_eq!(
            flash("FLASH (rx) : ORIGIN = 0x08008000, LENGTH = 0xF6000 /* was 0x100000 */"),
            invalid("LENGTH")
        );
    }

    #[test]
    fn refuses_signs_suffixes_and_missing_fields() {
        for (definition, field) in [
            ("FLASH (rx) : ORIGIN = 0x08008000, LENGTH = +16", "LENGTH"),
            ("FLASH (rx) : ORIGIN = 0x+08008000, LENGTH = 16", "ORIGIN"),
            ("FLASH (rx) : ORIGIN = 0x08008000, LENGTH = 984K", "LENGTH"),
            ("FLASH (rx) : ORIGIN = 0x08008000,", "LENGTH"),
            (
                "FLASH (rx) : LENGTH = 0xF6000, ORIGIN = 0x08008000",
                "LENGTH",
            ),
            ("FLASH (rx) : org = 0x08008000, len = 0xF6000", "ORIGIN"),
        ] {
            assert_eq!(flash(definition), invalid(field), "{definition}");
        }
    }

    #[test]
    fn refuses_an_empty_region_and_one_beyond_the_address_space() {
        for definition in [
            "FLASH (rx) : ORIGIN = 0x08008000, LENGTH = 0",
            "FLASH (rx) : ORIGIN = 0xFFFFF000, LENGTH = 0x1001",
        ] {
            assert_eq!(
                flash(definition),
                Err(LinkerScriptError::InvalidBounds("FLASH")),
                "{definition}"
            );
        }
    }

    #[test]
    fn skips_a_region_inside_a_comment() {
        let text = MEMORY_X.replace(
            "{\n",
            "{\n/* before the bootloader grew:\n  FLASH (rx) : ORIGIN = 0x08000000, LENGTH = \
             0x00100000\n*/\n",
        );
        assert_eq!(
            MemoryLayout::parse(&text).map(|layout| layout.flash),
            Ok(Region {
                start: 0x0800_7000,
                end: 0x080F_D000
            })
        );
        let only_commented = "/*\n  FLASH (rx) : ORIGIN = 0x08000000, LENGTH = 0x00100000\n*/\n";
        assert_eq!(
            MemoryLayout::parse(only_commented),
            Err(LinkerScriptError::MissingRegion("FLASH"))
        );
    }

    #[test]
    fn reads_the_ram_region_of_either_board() {
        assert_eq!(
            ram("RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 0x00030000"),
            Ok(Region {
                start: 0x2000_0000,
                end: 0x2003_0000
            })
        );
        assert_eq!(
            ram("RAM(rwx):ORIGIN=0x20000000,LENGTH=98304"),
            Ok(Region {
                start: 0x2000_0000,
                end: 0x2001_8000
            })
        );
        let ram_first = "MEMORY\n{\n  RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 0x00030000\n  \
                         FLASH (rx) : ORIGIN = 0x08008000, LENGTH = 0x000F6000\n}\n";
        assert_eq!(
            MemoryLayout::parse(ram_first).map(|layout| layout.ram.end),
            Ok(0x2003_0000)
        );
    }

    #[test]
    fn refuses_a_ram_region_given_as_an_expression_or_with_a_suffix() {
        for (definition, field) in [
            ("RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 192K", "LENGTH"),
            (
                "RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 0x30000 - 0x100",
                "LENGTH",
            ),
            (
                "RAM (rwx) : ORIGIN = 0x20000000 + 0x100, LENGTH = 0x30000",
                "LENGTH",
            ),
            ("RAM (rwx) : ORIGIN = SRAM_BASE, LENGTH = 0x30000", "ORIGIN"),
            ("RAM (rwx) : ORIGIN = 0x20000000", "LENGTH"),
        ] {
            assert_eq!(
                ram(definition),
                Err(LinkerScriptError::InvalidField {
                    region: "RAM",
                    field
                }),
                "{definition}"
            );
        }
    }

    #[test]
    fn refuses_an_empty_ram_region_and_one_beyond_the_address_space() {
        for definition in [
            "RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 0",
            "RAM (rwx) : ORIGIN = 0xFFFF0000, LENGTH = 0x10000",
        ] {
            assert_eq!(
                ram(definition),
                Err(LinkerScriptError::InvalidBounds("RAM")),
                "{definition}"
            );
        }
    }

    #[test]
    fn reads_only_the_region_named_ram() {
        assert_eq!(
            ram("CCMRAM (rwx) : ORIGIN = 0x10000000, LENGTH = 0x10000"),
            Err(LinkerScriptError::MissingRegion("RAM"))
        );
        assert_eq!(
            ram("RAM2 (rwx) : ORIGIN = 0x10000000, LENGTH = 0x10000"),
            Err(LinkerScriptError::MissingRegion("RAM"))
        );
        assert_eq!(
            ram(
                "CCMRAM (rwx) : ORIGIN = 0x10000000, LENGTH = 0x10000\n  RAM (rwx) : ORIGIN = \
                 0x20000000, LENGTH = 0x30000"
            ),
            Ok(Region {
                start: 0x2000_0000,
                end: 0x2003_0000
            })
        );
        assert_eq!(
            ram("/* RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 0x30000 */"),
            Err(LinkerScriptError::MissingRegion("RAM"))
        );
    }

    #[test]
    fn refuses_a_region_defined_twice() {
        assert_eq!(
            ram(
                "RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 0x30000\n  RAM (rwx) : ORIGIN = \
                 0x20000000, LENGTH = 0x18000"
            ),
            Err(LinkerScriptError::DuplicateRegion("RAM"))
        );
        assert_eq!(
            flash(
                "FLASH (rx) : ORIGIN = 0x08008000, LENGTH = 0xF6000\n  FLASH (rx) : ORIGIN = \
                   0x08000000, LENGTH = 0x100000"
            ),
            Err(LinkerScriptError::DuplicateRegion("FLASH"))
        );
    }
}
