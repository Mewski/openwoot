pub const MCU_ID_LEN: usize = 12;
pub const FLASH_UNIQUE_ID_LEN: usize = 8;
pub const FLASH_IDS_LEN: usize = 2 + FLASH_UNIQUE_ID_LEN;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FlashIds {
    pub manufacturer: u8,
    pub device: u8,
    pub unique_id: [u8; FLASH_UNIQUE_ID_LEN],
}

impl FlashIds {
    #[must_use]
    pub fn to_bytes(self) -> [u8; FLASH_IDS_LEN] {
        let mut bytes = [0; FLASH_IDS_LEN];
        let (ids, unique_id) = bytes.split_at_mut(2);
        ids.copy_from_slice(&[self.manufacturer, self.device]);
        unique_id.copy_from_slice(&self.unique_id);
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ids_go_out_as_manufacturer_device_and_unique_id() {
        let ids = FlashIds {
            manufacturer: 0xEF,
            device: 0x16,
            unique_id: [1, 2, 3, 4, 5, 6, 7, 8],
        };
        assert_eq!(ids.to_bytes(), [0xEF, 0x16, 1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
