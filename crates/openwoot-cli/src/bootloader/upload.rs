#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Upload {
    pub blocks: Vec<u8>,
    pub block_size: usize,
    pub crc: u32,
}

impl Upload {
    #[must_use]
    pub fn new(image: &[u8], block_size: usize) -> Self {
        let mut blocks = image.to_vec();
        blocks.resize(image.len().next_multiple_of(block_size.max(1)), 0xFF);
        let crc = crc24(&blocks);
        Self {
            blocks,
            block_size,
            crc,
        }
    }

    #[must_use]
    pub fn block_count(&self) -> usize {
        self.blocks.len() / self.block_size.max(1)
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

    #[test]
    fn crc24_matches_the_bootloader_test_values() {
        assert_eq!(crc24(&[1, 2, 3, 4]), 0x00_0001);
        assert_eq!(crc24(&(0..=255).collect::<Vec<u8>>()), 0xEF_3F62);
    }

    #[test]
    fn upload_pads_to_whole_blocks() {
        let upload = Upload::new(&[0; 1000], 256);
        assert_eq!(upload.blocks.len(), 1024);
        assert_eq!(upload.block_count(), 4);
        assert_eq!(upload.blocks[1000..], [0xFF; 24]);
    }
}
