use core::{ptr, slice};

use openwoot_image::Region;

pub const UNIQUE_ID_LEN: usize = 12;

const FLASH: Region = Region {
    start: 0x0800_0000,
    end: 0x0810_0000,
};
const UNIQUE_ID: usize = 0x1FFF_7A10;

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

#[must_use]
pub fn unique_id() -> [u8; UNIQUE_ID_LEN] {
    let mut id = [0; UNIQUE_ID_LEN];
    for (index, word) in id.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let address = ptr::with_exposed_provenance::<u32>(UNIQUE_ID + 4 * index);
        // SAFETY: the 96-bit unique id lies in the factory information block of the chip, which is
        // always readable, aligned to words and never written.
        word.copy_from_slice(&unsafe { address.read_volatile() }.to_le_bytes());
    }
    id
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
