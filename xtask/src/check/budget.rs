mod stack;

use std::fmt;

use anyhow::{Result, bail};
use openwoot_image::MemoryLayout;
use stack::StackBound;

use super::{call_graph::CallGraph, entry_points::EntryPoints};
use crate::elf::Elf;

const STACK_MARGIN: u32 = 2048;
const FLASH_MARGIN: u32 = 32 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Budget {
    pub(super) flash: u32,
    pub(super) flash_size: u32,
    pub(super) static_ram: u32,
    pub(super) stack: StackBound,
    pub(super) ram_size: u32,
}

impl Budget {
    pub(super) fn measure(
        elf: &Elf<'_>,
        layout: &MemoryLayout,
        graph: &CallGraph<'_>,
        entry_points: &EntryPoints,
    ) -> Result<Self> {
        let flash_end = elf
            .loadable_segments()?
            .iter()
            .map(|segment| {
                u32::try_from(segment.bytes.len()).map(|len| segment.address.saturating_add(len))
            })
            .try_fold(layout.flash.start, |end, segment_end| {
                Ok::<_, anyhow::Error>(end.max(segment_end?))
            })?;
        let static_ram = elf
            .allocated_sections()
            .iter()
            .filter(|section| layout.ram.contains(section.address))
            .fold(0_u32, |total, section| total.saturating_add(section.size));
        Ok(Self {
            flash: flash_end - layout.flash.start,
            flash_size: layout.flash.size(),
            static_ram,
            stack: stack::bound(graph, entry_points)?,
            ram_size: layout.ram.size(),
        })
    }

    pub(super) fn check(&self) -> Result<()> {
        let ram_needed = self
            .static_ram
            .saturating_add(self.stack.total())
            .saturating_add(STACK_MARGIN);
        if ram_needed > self.ram_size {
            bail!(
                "the image needs {} bytes of statics, a stack of up to {} bytes ({} bytes thread \
                 mode, {} bytes interrupts) and a margin of {STACK_MARGIN} bytes, more than the {} \
                 bytes of RAM",
                self.static_ram,
                self.stack.total(),
                self.stack.thread,
                self.stack.interrupts,
                self.ram_size
            );
        }
        if self.flash.saturating_add(FLASH_MARGIN) > self.flash_size {
            bail!(
                "the image area holds {} bytes, the image takes {} bytes and leaves less than \
                 the margin of {FLASH_MARGIN} bytes",
                self.flash_size,
                self.flash
            );
        }
        Ok(())
    }
}

impl fmt::Display for Budget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "flash {} of {} bytes, RAM {} bytes static + {} bytes stack ({} thread, {} \
             interrupts) of {} bytes",
            self.flash,
            self.flash_size,
            self.static_ram,
            self.stack.total(),
            self.stack.thread,
            self.stack.interrupts,
            self.ram_size
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget(static_ram: u32, thread: u32, flash: u32) -> Budget {
        Budget {
            flash,
            flash_size: 0x000F_6000,
            static_ram,
            stack: StackBound {
                thread,
                interrupts: 10_000,
            },
            ram_size: 0x0001_8000,
        }
    }

    #[test]
    fn statics_and_stack_within_ram_pass() {
        assert!(budget(65_504, 20_000, 700_000).check().is_ok());
    }

    #[test]
    fn a_stack_beyond_ram_is_reported() {
        assert_eq!(
            budget(65_504, 96_720, 700_000)
                .check()
                .unwrap_err()
                .to_string(),
            "the image needs 65504 bytes of statics, a stack of up to 106720 bytes (96720 bytes \
             thread mode, 10000 bytes interrupts) and a margin of 2048 bytes, more than the 98304 \
             bytes of RAM"
        );
    }

    #[test]
    fn an_image_without_flash_margin_is_reported() {
        assert!(budget(1000, 1000, 0x000F_0000).check().is_err());
    }
}
