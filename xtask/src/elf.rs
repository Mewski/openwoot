use anyhow::{Context, Result, anyhow};
use object::{
    LittleEndian,
    elf::{FileHeader32, PT_LOAD},
    read::elf::{FileHeader, ProgramHeader, SectionHeader},
};

pub(crate) struct Elf<'a> {
    data: &'a [u8],
    header: &'a FileHeader32<LittleEndian>,
    endian: LittleEndian,
}

pub(crate) struct Segment<'a> {
    pub(crate) address: u32,
    pub(crate) bytes: &'a [u8],
}

pub(crate) struct Section<'a> {
    pub(crate) address: u32,
    pub(crate) bytes: &'a [u8],
}

impl<'a> Elf<'a> {
    pub(crate) fn parse(data: &'a [u8]) -> Result<Self> {
        let header = FileHeader32::<LittleEndian>::parse(data).context("not a 32-bit ELF file")?;
        let endian = header.endian().context("not a little-endian ELF file")?;
        Ok(Self {
            data,
            header,
            endian,
        })
    }

    pub(crate) fn entry(&self) -> u32 {
        self.header.e_entry(self.endian)
    }

    pub(crate) fn loadable_segments(&self) -> Result<Vec<Segment<'a>>> {
        let mut segments = self
            .header
            .program_headers(self.endian, self.data)?
            .iter()
            .filter(|header| {
                header.p_type(self.endian) == PT_LOAD && header.p_filesz(self.endian) > 0
            })
            .map(|header| {
                let bytes = header
                    .data(self.endian, self.data)
                    .map_err(|()| anyhow!("a loadable segment lies outside the file"))?;
                Ok(Segment {
                    address: header.p_paddr(self.endian),
                    bytes,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        segments.sort_by_key(|segment| segment.address);
        Ok(segments)
    }

    pub(crate) fn section(&self, name: &str) -> Result<Option<Section<'a>>> {
        let sections = self.header.sections(self.endian, self.data)?;
        let Some((_, header)) = sections.section_by_name(self.endian, name.as_bytes()) else {
            return Ok(None);
        };
        let bytes = header.data(self.endian, self.data)?;
        Ok(Some(Section {
            address: header.sh_addr(self.endian),
            bytes,
        }))
    }
}
