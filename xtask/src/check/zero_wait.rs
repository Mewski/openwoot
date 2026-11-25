use openwoot_image::{Hex, Region};

use crate::elf::Elf;

const CODE_SECTIONS: [&str; 2] = [".vector_table", ".text"];

pub(super) fn code_beyond(elf: &Elf<'_>, zero_wait: Region) -> Vec<String> {
    CODE_SECTIONS
        .iter()
        .filter_map(|&name| {
            let section = elf
                .allocated_sections()
                .iter()
                .find(|section| section.name == name)?;
            let end = section.address.saturating_add(section.size);
            outside(zero_wait, section.address, end).then(|| problem(name, end, zero_wait))
        })
        .collect()
}

const fn outside(zero_wait: Region, start: u32, end: u32) -> bool {
    start < zero_wait.start || end > zero_wait.end
}

fn problem(section: &str, end: u32, zero_wait: Region) -> String {
    format!(
        "`{section}` ends at {}, beyond the zero-wait flash that ends at {} (`ZERO_WAIT` in \
         `memory.x`)",
        Hex(end),
        Hex(zero_wait.end)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZERO_WAIT: Region = Region {
        start: 0x0800_0000,
        end: 0x0808_0000,
    };

    #[test]
    fn code_that_ends_at_the_zero_wait_end_lies_inside() {
        assert!(!outside(ZERO_WAIT, 0x0800_8208, 0x0808_0000));
        assert!(!outside(ZERO_WAIT, 0x0800_8000, 0x0800_81AC));
    }

    #[test]
    fn code_that_crosses_or_lies_beyond_the_end_lies_outside() {
        assert!(outside(ZERO_WAIT, 0x0800_8208, 0x0808_0001));
        assert!(outside(ZERO_WAIT, 0x0808_5D60, 0x0809_0000));
        assert!(outside(ZERO_WAIT, 0x07FF_FFF0, 0x0800_0010));
    }

    #[test]
    fn the_problem_names_the_section_and_both_ends() {
        assert_eq!(
            problem(
                ".text",
                0x0806_F918,
                Region {
                    start: 0x0800_0000,
                    end: 0x0804_0000,
                }
            ),
            "`.text` ends at 0x0806_f918, beyond the zero-wait flash that ends at 0x0804_0000 \
             (`ZERO_WAIT` in `memory.x`)"
        );
    }
}
