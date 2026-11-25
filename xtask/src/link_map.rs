use anyhow::{Context, Result};

const HEADER_OUT: &str = "Out";
const HEADER_IN: &str = " In ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InputSection {
    pub(crate) name: String,
    pub(crate) address: u32,
    pub(crate) size: u32,
}

pub(crate) fn input_sections(map: &str) -> Result<Vec<InputSection>> {
    let mut lines = map.lines();
    let header = lines.next().context("the link map is empty")?;
    let fields_end = header
        .find(HEADER_OUT)
        .context("the link map has no `Out` column")?;
    let name_start = header
        .find(HEADER_IN)
        .map(|position| position + 1)
        .context("the link map has no `In` column")?;
    Ok(lines
        .filter_map(|line| input_section(line, fields_end, name_start))
        .collect())
}

fn input_section(line: &str, fields_end: usize, name_start: usize) -> Option<InputSection> {
    let fields = line.get(..fields_end)?;
    let indent = line.get(fields_end..name_start)?;
    let name = line.get(name_start..)?;
    if !indent.chars().all(|character| character == ' ') || name.starts_with(' ') {
        return None;
    }
    let mut fields = fields.split_whitespace();
    let address = u32::from_str_radix(fields.next()?, 16).ok()?;
    let _load_address = fields.next()?;
    let size = u32::from_str_radix(fields.next()?, 16).ok()?;
    let (_, section) = name.rsplit_once(":(")?;
    Some(InputSection {
        name: section.strip_suffix(')')?.to_owned(),
        address,
        size,
    })
    .filter(|section| section.size > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "     VMA      LMA     Size Align Out     In      Symbol
       0        0        0     1 _stext = ALIGN(ADDR(.app_descriptor) + SIZEOF(.app_descriptor), 8)
 80558e0  80558e0    16cac    16 .rodata
 8055926  8055926     151d     1         <internal>:(.rodata.str1.1)
 8067db4  8067db4       10     4         /out/firmware.rcgu.o:(.rodata..Lanon.62d5.165)
 8067db4  8067db4       10     1                 .Lanon.62d5.165
 8067de4  8067de4       10     4         /out/firmware.rcgu.o:(.rodata..Lanon.62d5.168)
 8067df4  8067df4        0     4         /out/firmware.rcgu.o:(.rodata..Lanon.62d5.169)
";

    #[test]
    fn input_sections_are_read_with_their_address_and_size() {
        let sections = input_sections(MAP).unwrap();
        let section = |name: &str, address, size| InputSection {
            name: name.to_owned(),
            address,
            size,
        };
        assert_eq!(
            sections,
            [
                section(".rodata.str1.1", 0x0805_5926, 0x151D),
                section(".rodata..Lanon.62d5.165", 0x0806_7DB4, 0x10),
                section(".rodata..Lanon.62d5.168", 0x0806_7DE4, 0x10),
            ]
        );
    }

    #[test]
    fn a_map_without_its_header_is_rejected() {
        assert!(input_sections("").is_err());
        assert!(input_sections("VMA LMA Size\n").is_err());
    }
}
