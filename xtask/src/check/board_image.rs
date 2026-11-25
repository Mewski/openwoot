use std::fs;

use anyhow::{Context, Result, bail};
use openwoot_image::{Descriptor, Hex, Image, MemoryLayout};

use super::{
    budget::Budget,
    panic_paths::{self, Findings},
    scan_interrupt, zero_wait,
};
use crate::{
    call_graph::CallGraph,
    elf::Elf,
    entry_points::EntryPoints,
    firmware,
    workspace::{Board, Workspace},
};

const VECTOR_SIZE: usize = 4;

pub(super) struct Analysis {
    pub(super) budget: Budget,
    pub(super) panics: Findings,
    pub(super) critical_sections: Vec<String>,
    pub(super) code_beyond_zero_wait: Vec<String>,
}

pub(super) fn check(workspace: &Workspace, board: &Board) -> Result<Analysis> {
    let images = firmware::build(workspace, board)?;
    let memory_x_path = board.directory.join("memory.x");
    let memory_x = fs::read_to_string(&memory_x_path)
        .with_context(|| format!("could not read `{}`", memory_x_path.display()))?;
    let layout = MemoryLayout::parse(&memory_x)?;
    let file = fs::read(&images.elf)
        .with_context(|| format!("could not read `{}`", images.elf.display()))?;
    let elf = Elf::parse(&file)?;

    let vectors = elf
        .section(".vector_table")
        .context("the ELF file has no `.vector_table` section")?;
    if vectors.address != layout.flash.start {
        bail!(
            "the vector table lies at {}, `memory.x` puts `FLASH` at {}",
            Hex(vectors.address),
            Hex(layout.flash.start)
        );
    }
    let stack_top = vectors
        .bytes
        .first_chunk()
        .map(|bytes| u32::from_le_bytes(*bytes));
    if stack_top != Some(layout.ram.end) {
        bail!(
            "the initial stack pointer is {}, `memory.x` puts the end of `RAM` at {}",
            stack_top.map_or_else(|| "missing".to_owned(), |value| Hex(value).to_string()),
            Hex(layout.ram.end)
        );
    }

    let section = elf
        .section(".app_descriptor")
        .context("the ELF file has no `.app_descriptor` section")?;
    descriptor_placement(
        layout.flash.start,
        vectors.bytes.len(),
        section.address,
        section.bytes.len(),
    )?;
    let descriptor = Descriptor::from_bytes(section.bytes)?;
    if descriptor.image() != layout.flash {
        bail!(
            "the descriptor names the image area {}-{}, `memory.x` puts `FLASH` at {}-{}",
            Hex(descriptor.image().start),
            Hex(descriptor.image().end),
            Hex(layout.flash.start),
            Hex(layout.flash.end)
        );
    }
    if descriptor.ram() != layout.ram {
        bail!(
            "the descriptor names the RAM {}-{}, `memory.x` puts `RAM` at {}-{}",
            Hex(descriptor.ram().start),
            Hex(descriptor.ram().end),
            Hex(layout.ram.start),
            Hex(layout.ram.end)
        );
    }

    for segment in elf.loadable_segments()? {
        let end = u64::from(segment.address) + u64::try_from(segment.bytes.len())?;
        if !layout.flash.contains(segment.address) || end > u64::from(layout.flash.end) {
            bail!(
                "loadable data at {} lies outside `FLASH` of `memory.x`",
                Hex(segment.address)
            );
        }
    }

    let binary = fs::read(&images.bin)
        .with_context(|| format!("could not read `{}`", images.bin.display()))?;
    let image = Image::new(&binary, Some(layout.flash.start))?;
    if *image.descriptor() != descriptor {
        bail!("the descriptor found in the binary differs from the ELF section");
    }

    let graph = CallGraph::new(&elf)?;
    let entry_points = EntryPoints::find(&elf, &graph)?;
    Ok(Analysis {
        budget: Budget::measure(&elf, &layout, &graph, &entry_points)?,
        panics: panic_paths::find(&elf, &graph, &entry_points),
        critical_sections: scan_interrupt::critical_sections(&graph, &entry_points)?,
        code_beyond_zero_wait: zero_wait::code_beyond(&graph, &entry_points, layout.zero_wait),
    })
}

fn descriptor_placement(
    flash_start: u32,
    vector_table_len: usize,
    descriptor_address: u32,
    descriptor_len: usize,
) -> Result<()> {
    if descriptor_len != Descriptor::ENCODED_LEN {
        bail!(
            "the `.app_descriptor` section holds {descriptor_len} bytes, a descriptor has {}",
            Descriptor::ENCODED_LEN
        );
    }
    if !vector_table_len.is_multiple_of(VECTOR_SIZE) {
        bail!(
            "the vector table is {vector_table_len} bytes long, not a whole number of \
             {VECTOR_SIZE}-byte vectors"
        );
    }
    let Some(offset) = descriptor_address.checked_sub(flash_start) else {
        bail!(
            "the descriptor lies at {}, below `FLASH` of `memory.x` at {}",
            Hex(descriptor_address),
            Hex(flash_start)
        );
    };
    let offset = usize::try_from(offset)?;
    let expected_offset = vector_table_len.next_multiple_of(Descriptor::ALIGNMENT);
    if offset != expected_offset {
        bail!(
            "the descriptor lies {offset} bytes into the image, expected {expected_offset} \
             bytes, behind the vector table"
        );
    }
    if offset > Descriptor::MAX_OFFSET {
        bail!(
            "the descriptor lies {offset} bytes into the image, beyond the first {} bytes \
             where the image checks look for it",
            Descriptor::MAX_OFFSET
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLASH_START: u32 = 0x0800_8000;
    const LEN: usize = Descriptor::ENCODED_LEN;

    fn message(result: Result<()>) -> String {
        result.unwrap_err().to_string()
    }

    #[test]
    fn descriptor_behind_the_vector_table_is_accepted() {
        assert!(descriptor_placement(FLASH_START, 0x1B0, FLASH_START + 0x1C0, LEN).is_ok());
    }

    #[test]
    fn descriptor_below_flash_is_reported() {
        assert_eq!(
            message(descriptor_placement(FLASH_START, 0x1B0, 0x0800_0000, LEN)),
            "the descriptor lies at 0x0800_0000, below `FLASH` of `memory.x` at 0x0800_8000"
        );
    }

    #[test]
    fn vector_table_of_partial_vectors_is_reported() {
        assert_eq!(
            message(descriptor_placement(
                FLASH_START,
                0x1B2,
                FLASH_START + 0x1C0,
                LEN
            )),
            "the vector table is 434 bytes long, not a whole number of 4-byte vectors"
        );
    }

    #[test]
    fn descriptor_away_from_the_vector_table_is_reported() {
        assert_eq!(
            message(descriptor_placement(
                FLASH_START,
                0x1B0,
                FLASH_START + 0x200,
                LEN
            )),
            "the descriptor lies 512 bytes into the image, expected 448 bytes, behind the vector \
             table"
        );
    }

    #[test]
    fn descriptor_beyond_the_search_window_is_reported() {
        assert_eq!(
            message(descriptor_placement(
                FLASH_START,
                0x410,
                FLASH_START + 0x440,
                LEN
            )),
            "the descriptor lies 1088 bytes into the image, beyond the first 1024 bytes where the \
             image checks look for it"
        );
    }

    #[test]
    fn section_of_another_length_than_a_descriptor_is_reported() {
        assert_eq!(
            message(descriptor_placement(
                FLASH_START,
                0x1B0,
                FLASH_START + 0x1C0,
                60
            )),
            format!("the `.app_descriptor` section holds 60 bytes, a descriptor has {LEN}")
        );
    }
}
