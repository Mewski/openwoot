mod relocation;
mod symbol;

use anyhow::{Context, Result, anyhow};
use object::{
    LittleEndian,
    elf::{
        FileHeader32, PT_LOAD, R_ARM_ABS32, R_ARM_THM_JUMP19, R_ARM_THM_JUMP24, R_ARM_THM_MOVT_ABS,
        R_ARM_THM_MOVW_ABS_NC, R_ARM_THM_PC22, SHF_ALLOC, SHT_NOBITS, SHT_SYMTAB, STT_FUNC,
        STT_NOTYPE, STT_OBJECT,
    },
    read::elf::{FileHeader, ProgramHeader, SectionHeader, Sym},
};
pub(crate) use relocation::{Relocation, RelocationKind};
use rustc_demangle::demangle;
pub(crate) use symbol::{Symbol, SymbolKind};

pub(crate) struct Elf<'a> {
    file: &'a [u8],
    header: &'a FileHeader32<LittleEndian>,
    endian: LittleEndian,
    allocated: Vec<Allocated<'a>>,
}

pub(crate) struct Segment<'a> {
    pub(crate) address: u32,
    pub(crate) bytes: &'a [u8],
}

pub(crate) struct Section<'a> {
    pub(crate) address: u32,
    pub(crate) bytes: &'a [u8],
}

pub(crate) struct Allocated<'a> {
    pub(crate) name: String,
    pub(crate) address: u32,
    pub(crate) size: u32,
    pub(crate) bytes: Option<&'a [u8]>,
}

impl<'a> Elf<'a> {
    pub(crate) fn parse(file: &'a [u8]) -> Result<Self> {
        let header = FileHeader32::<LittleEndian>::parse(file).context("not a 32-bit ELF file")?;
        let endian = header.endian().context("not a little-endian ELF file")?;
        let sections = header.sections(endian, file)?;
        let mut allocated = Vec::new();
        for header in sections.iter() {
            if !header.sh_flags(endian).contains(SHF_ALLOC) || header.sh_size(endian) == 0 {
                continue;
            }
            let bytes = if header.sh_type(endian) == SHT_NOBITS {
                None
            } else {
                Some(header.data(endian, file)?)
            };
            allocated.push(Allocated {
                name: String::from_utf8_lossy(sections.section_name(endian, header)?).into_owned(),
                address: header.sh_addr(endian),
                size: header.sh_size(endian),
                bytes,
            });
        }
        allocated.sort_by_key(|section| section.address);
        Ok(Self {
            file,
            header,
            endian,
            allocated,
        })
    }

    pub(crate) fn entry(&self) -> u32 {
        self.header.e_entry(self.endian)
    }

    pub(crate) fn loadable_segments(&self) -> Result<Vec<Segment<'a>>> {
        let mut segments = self
            .header
            .program_headers(self.endian, self.file)?
            .iter()
            .filter(|header| {
                header.p_type(self.endian) == PT_LOAD && header.p_filesz(self.endian) > 0
            })
            .map(|header| {
                let bytes = header
                    .data(self.endian, self.file)
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

    pub(crate) fn section(&self, name: &str) -> Option<Section<'a>> {
        let section = self.allocated.iter().find(|section| section.name == name)?;
        Some(Section {
            address: section.address,
            bytes: section.bytes?,
        })
    }

    pub(crate) fn allocated_sections(&self) -> &[Allocated<'a>] {
        &self.allocated
    }

    pub(crate) fn section_at(&self, address: u32) -> Option<&Allocated<'a>> {
        self.allocated
            .iter()
            .find(|section| section.contains(address))
    }

    pub(crate) fn bytes_at(&self, address: u32, len: usize) -> Option<&'a [u8]> {
        let section = self.section_at(address)?;
        let start = usize::try_from(address - section.address).ok()?;
        section.bytes?.get(start..start.checked_add(len)?)
    }

    pub(crate) fn word_at(&self, address: u32) -> Option<u32> {
        self.bytes_at(address, 4)?
            .first_chunk()
            .map(|bytes| u32::from_le_bytes(*bytes))
    }

    pub(crate) fn symbols(&self) -> Result<Vec<Symbol>> {
        let sections = self.header.sections(self.endian, self.file)?;
        let table = sections.symbols(self.endian, self.file, SHT_SYMTAB)?;
        let mut symbols = Vec::new();
        for entry in table.iter() {
            let name = entry.name(self.endian, table.strings())?;
            if name.is_empty() || entry.is_undefined(self.endian) {
                continue;
            }
            let name = String::from_utf8_lossy(name);
            let kind = match (entry.st_type(), name.as_ref()) {
                (STT_NOTYPE, "$t") => SymbolKind::CodeStart,
                (STT_NOTYPE, "$d") => SymbolKind::LiteralsStart,
                (STT_FUNC, _) => SymbolKind::Function,
                (STT_OBJECT | STT_NOTYPE, _) if !name.starts_with('$') => SymbolKind::Object,
                _ => continue,
            };
            let value = entry.st_value(self.endian);
            let address = if kind == SymbolKind::Function {
                value & !1
            } else {
                value
            };
            if self.bytes_at(address, 0).is_none() && !self.is_uninitialised(address) {
                continue;
            }
            symbols.push(Symbol {
                name: format!("{:#}", demangle(&name)),
                address,
                size: entry.st_size(self.endian),
                kind,
            });
        }
        Ok(symbols)
    }

    pub(crate) fn relocations(&self) -> Result<Vec<Relocation>> {
        let sections = self.header.sections(self.endian, self.file)?;
        let mut relocations = Vec::new();
        for header in sections.iter() {
            let Some((entries, _)) = header.rel(self.endian, self.file)? else {
                continue;
            };
            let target = sections.section(object::SectionIndex(usize::try_from(
                header.sh_info(self.endian),
            )?))?;
            if !target.sh_flags(self.endian).contains(SHF_ALLOC) {
                continue;
            }
            for entry in entries {
                let kind = match entry.r_type(self.endian) {
                    R_ARM_ABS32 => RelocationKind::Absolute,
                    R_ARM_THM_PC22 | R_ARM_THM_JUMP24 => RelocationKind::Branch,
                    R_ARM_THM_JUMP19 => RelocationKind::ConditionalBranch,
                    R_ARM_THM_MOVW_ABS_NC => RelocationKind::MoveLow,
                    R_ARM_THM_MOVT_ABS => RelocationKind::MoveHigh,
                    _ => continue,
                };
                relocations.push(Relocation {
                    offset: entry.r_offset.get(self.endian),
                    kind,
                });
            }
        }
        relocations.sort_by_key(|relocation| relocation.offset);
        Ok(relocations)
    }

    fn is_uninitialised(&self, address: u32) -> bool {
        self.section_at(address)
            .is_some_and(|section| section.bytes.is_none())
    }
}

impl Allocated<'_> {
    fn contains(&self, address: u32) -> bool {
        address >= self.address && address - self.address < self.size
    }
}
