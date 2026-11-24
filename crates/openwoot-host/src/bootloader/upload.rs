use openwoot_image::Region;

use super::{BLOCK_LEN, Error, FIRST_BLOCK_ADDRESS, MAX_BLOCKS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upload {
    pub bytes: Vec<u8>,
    pub block_size: usize,
    pub crc: u32,
}

impl Upload {
    #[must_use]
    pub fn new(image: &[u8], block_size: usize) -> Self {
        let block_size = block_size.max(1);
        let mut bytes = image.to_vec();
        bytes.resize(image.len().next_multiple_of(block_size), 0xFF);
        let crc = crc24(&bytes);
        Self {
            bytes,
            block_size,
            crc,
        }
    }

    pub fn within(image: &[u8], block_size: u32, area: Region) -> Result<Self, Error> {
        if usize::try_from(block_size) != Ok(BLOCK_LEN) {
            return Err(Error::BlockSizeUnsupported(block_size));
        }
        if area.start != FIRST_BLOCK_ADDRESS {
            return Err(Error::MisplacedImageArea(area.start));
        }
        let len = image.len().next_multiple_of(BLOCK_LEN);
        if !u32::try_from(len).is_ok_and(|len| len <= area.size()) {
            return Err(Error::BeyondImageArea {
                len,
                start: area.start,
                end: area.end,
                capacity: area.size(),
            });
        }
        let blocks = len / BLOCK_LEN;
        if !(1..=MAX_BLOCKS).contains(&blocks) {
            return Err(Error::BlockCountOutOfRange(blocks));
        }
        Ok(Self::new(image, BLOCK_LEN))
    }

    #[must_use]
    pub fn block_count(&self) -> usize {
        self.bytes.len() / self.block_size
    }

    pub fn blocks(&self) -> impl Iterator<Item = &[u8]> {
        self.bytes.chunks(self.block_size)
    }
}

#[must_use]
pub fn crc24(bytes: &[u8]) -> u32 {
    bytes.chunks(2).fold(0, |crc, pair| {
        let word = pair
            .iter()
            .rev()
            .fold(0, |word, &byte| word << 8 | u32::from(byte));
        let feedback = if crc & 0x80_0000 == 0 { 0 } else { 0x80_001B };
        ((crc << 1) & 0xFF_FFFE ^ word ^ feedback) & 0xFF_FFFF
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Region = Region {
        start: 0x0800_8000,
        end: 0x0800_8300,
    };
    const WHOLE_APPLICATION_AREA: Region = Region {
        start: 0x0800_8000,
        end: 0x0A00_0000,
    };

    #[test]
    fn crc24_matches_the_bootloader_test_values() {
        assert_eq!(crc24(&[1, 2, 3, 4]), 0x00_0001);
        assert_eq!(crc24(&(0..=255).collect::<Vec<u8>>()), 0xEF_3F62);
    }

    #[test]
    fn upload_pads_to_whole_blocks() {
        let upload = Upload::new(&[0; 1000], 256);
        assert_eq!(upload.bytes.len(), 1024);
        assert_eq!(upload.block_count(), 4);
        assert_eq!(upload.bytes[1000..], [0xFF; 24]);
        assert!(upload.blocks().all(|block| block.len() == 256));
    }

    #[test]
    fn an_upload_that_fills_its_image_area_is_accepted() {
        let image = [0x5A; 0x2F0];
        let upload = Upload::within(&image, 256, AREA).unwrap();
        assert_eq!(upload, Upload::new(&image, 256));
        assert_eq!(upload.block_count(), 3);
    }

    #[test]
    fn padding_past_the_image_area_is_refused() {
        let area = Region {
            start: 0x0800_8000,
            end: 0x0800_82F0,
        };
        assert_eq!(
            Upload::within(&[0x5A; 0x2F0], 256, area),
            Err(Error::BeyondImageArea {
                len: 0x300,
                start: 0x0800_8000,
                end: 0x0800_82F0,
                capacity: 0x2F0,
            })
        );
        assert!(matches!(
            Upload::within(&[0x5A; 0x301], 256, AREA),
            Err(Error::BeyondImageArea { len: 0x400, .. })
        ));
    }

    #[test]
    fn a_block_size_the_bootloader_does_not_take_is_refused() {
        for block_size in [1, 255, 257, 512, u32::MAX] {
            assert_eq!(
                Upload::within(&[0x5A; 0x300], block_size, AREA),
                Err(Error::BlockSizeUnsupported(block_size))
            );
        }
    }

    #[test]
    fn an_empty_upload_or_one_past_the_last_block_of_the_bootloader_is_refused() {
        assert_eq!(
            Upload::within(&[], 256, WHOLE_APPLICATION_AREA),
            Err(Error::BlockCountOutOfRange(0))
        );
        assert_eq!(
            Upload::within(
                &vec![0; MAX_BLOCKS * BLOCK_LEN + 1],
                256,
                WHOLE_APPLICATION_AREA
            ),
            Err(Error::BlockCountOutOfRange(MAX_BLOCKS + 1))
        );
        let largest = Upload::within(
            &vec![0; MAX_BLOCKS * BLOCK_LEN],
            256,
            WHOLE_APPLICATION_AREA,
        )
        .unwrap();
        assert_eq!(largest.block_count(), MAX_BLOCKS);
    }

    #[test]
    fn the_largest_upload_ends_where_the_serial_page_begins() {
        let end = u32::try_from(MAX_BLOCKS * BLOCK_LEN).unwrap() + FIRST_BLOCK_ADDRESS;
        assert_eq!(end, 0x080F_E000);
    }

    #[test]
    fn an_image_area_the_bootloader_does_not_write_at_is_refused() {
        let area = Region {
            start: 0x0800_0000,
            end: 0x0800_8000,
        };
        assert_eq!(
            Upload::within(&[0x5A; 0x300], 256, area),
            Err(Error::MisplacedImageArea(0x0800_0000))
        );
    }
}
