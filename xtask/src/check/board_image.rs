use std::fs;

use anyhow::{Context, Result, bail};
use openwoot_image::{Descriptor, Hex, Image, MemoryLayout};

use crate::{
    elf::Elf,
    firmware,
    workspace::{Board, Workspace},
};

const VECTOR_SIZE: usize = 4;

pub(super) fn check(workspace: &Workspace, board: &Board) -> Result<()> {
    let images = firmware::build(workspace, board)?;
    let memory_x_path = board.directory.join("memory.x");
    let memory_x = fs::read_to_string(&memory_x_path)
        .with_context(|| format!("could not read `{}`", memory_x_path.display()))?;
    let layout = MemoryLayout::parse(&memory_x)?;
    let data = fs::read(&images.elf)
        .with_context(|| format!("could not read `{}`", images.elf.display()))?;
    let elf = Elf::parse(&data)?;

    let vectors = elf
        .section(".vector_table")?
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
        .section(".app_descriptor")?
        .context("the ELF file has no `.app_descriptor` section")?;
    let offset = usize::try_from(section.address - layout.flash.start)?;
    let expected_offset = vectors.bytes.len().next_multiple_of(Descriptor::ALIGNMENT);
    if offset != expected_offset
        || offset > Descriptor::MAX_OFFSET
        || vectors.bytes.len() % VECTOR_SIZE != 0
    {
        bail!(
            "the descriptor lies {offset} bytes into the image, expected {expected_offset} \
             bytes, behind the vector table"
        );
    }
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
    Ok(())
}
