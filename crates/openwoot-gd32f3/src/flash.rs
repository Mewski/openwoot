use core::{ptr, slice};

use openwoot_image::Region;

const FLASH: Region = Region {
    start: 0x0800_0000,
    end: 0x0810_0000,
};

#[must_use]
pub fn read(region: Region) -> Option<&'static [u8]> {
    let len = usize::try_from(region.end.checked_sub(region.start)?).ok()?;
    if !contains(region) {
        return None;
    }
    let start = ptr::with_exposed_provenance::<u8>(usize::try_from(region.start).ok()?);
    // SAFETY: `region` lies inside the memory-mapped flash of the chip, which is readable at every
    // address and never changes while the firmware runs, because nothing in the firmware erases or
    // programs it.
    Some(unsafe { slice::from_raw_parts(start, len) })
}

const fn contains(region: Region) -> bool {
    FLASH.start <= region.start && region.start <= region.end && region.end <= FLASH.end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_regions_inside_the_flash_are_readable() {
        let inside = Region {
            start: 0x080F_E000,
            end: 0x080F_F000,
        };
        assert!(contains(inside));
        assert!(contains(FLASH));
        for outside in [
            Region {
                start: 0x07FF_FFFF,
                end: 0x0800_0010,
            },
            Region {
                start: 0x080F_F000,
                end: 0x0810_0001,
            },
            Region {
                start: 0x2000_0000,
                end: 0x2000_0010,
            },
            Region {
                start: 0x080F_F000,
                end: 0x080F_E000,
            },
        ] {
            assert!(!contains(outside), "{outside:?}");
            assert_eq!(read(outside), None);
        }
    }
}
