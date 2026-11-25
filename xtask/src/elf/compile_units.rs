use anyhow::{Context, Result};
use gimli::{Dwarf, EndianSlice, LittleEndian};

use super::Elf;

const CRATE_MARKER: &str = "/@/";

pub(crate) struct CompileUnits {
    ranges: Vec<UnitRange>,
}

struct UnitRange {
    start: u32,
    end: u32,
    crate_name: String,
}

impl CompileUnits {
    pub(crate) fn read(elf: &Elf<'_>) -> Result<Self> {
        let dwarf = Dwarf::load(|section| {
            elf.section_data(section.name())
                .map(|data| EndianSlice::new(data.unwrap_or_default(), LittleEndian))
        })?;
        let mut ranges = Vec::new();
        let mut headers = dwarf.units();
        while let Some(header) = headers
            .next()
            .context("could not read the DWARF unit headers")?
        {
            let unit = dwarf
                .unit(header)
                .context("could not read a DWARF compile unit")?;
            let Some(name) = unit.name else {
                continue;
            };
            let name = name.to_string_lossy();
            let Some(crate_name) = crate_of(&name) else {
                continue;
            };
            let mut unit_ranges = dwarf
                .unit_ranges(&unit)
                .with_context(|| format!("could not read the ranges of `{name}`"))?;
            while let Some(range) = unit_ranges
                .next()
                .with_context(|| format!("could not read the ranges of `{name}`"))?
            {
                ranges.push(UnitRange {
                    start: u32::try_from(range.begin)?,
                    end: u32::try_from(range.end)?,
                    crate_name: crate_name.to_owned(),
                });
            }
        }
        ranges.sort_by_key(|range| range.start);
        Ok(Self { ranges })
    }

    pub(crate) fn crate_at(&self, address: u32) -> Option<&str> {
        let following = self.ranges.partition_point(|range| range.start <= address);
        let range = self.ranges.get(following.checked_sub(1)?)?;
        (address < range.end).then_some(range.crate_name.as_str())
    }
}

fn crate_of(unit_name: &str) -> Option<&str> {
    let (_, unit) = unit_name.rsplit_once(CRATE_MARKER)?;
    unit.split('.').next().filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crate_is_the_name_before_the_codegen_unit() {
        assert_eq!(
            crate_of("boards/wooting-80he/firmware/src/main.rs/@/openwoot_wooting_80he.26fc-cgu.0"),
            Some("openwoot_wooting_80he")
        );
        assert_eq!(
            crate_of(
                "/rustc/0ef1/library/compiler-builtins/src/lib.rs/@/compiler_builtins.475d-cgu.002"
            ),
            Some("compiler_builtins")
        );
        assert_eq!(crate_of("src/startup.S"), None);
    }

    #[test]
    fn an_address_belongs_to_the_unit_whose_range_holds_it() {
        let units = CompileUnits {
            ranges: vec![
                UnitRange {
                    start: 0x0800_8000,
                    end: 0x0800_8100,
                    crate_name: "board".to_owned(),
                },
                UnitRange {
                    start: 0x0800_8200,
                    end: 0x0800_8300,
                    crate_name: "firmware".to_owned(),
                },
            ],
        };
        assert_eq!(units.crate_at(0x0800_8000), Some("board"));
        assert_eq!(units.crate_at(0x0800_80FF), Some("board"));
        assert_eq!(units.crate_at(0x0800_8100), None);
        assert_eq!(units.crate_at(0x0800_8250), Some("firmware"));
        assert_eq!(units.crate_at(0x0800_7FFF), None);
    }
}
