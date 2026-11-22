use core::mem::size_of;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbIds {
    pub vendor_id: u16,
    pub product_id_base: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateMode {
    pub product_id: u16,
    pub block_size: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub start: u32,
    pub end: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Descriptor {
    magic: [u8; 8],
    format_version: u16,
    usb_vendor_id: u16,
    usb_product_id_base: u16,
    update_product_id: u16,
    image_start: u32,
    image_end: u32,
    update_block_size: u32,
    board_name: [u8; Descriptor::BOARD_NAME_CAPACITY],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DescriptorError {
    #[error("the descriptor is truncated")]
    Truncated,
    #[error("the descriptor does not start with its magic bytes")]
    Magic,
    #[error("descriptor format version {0} is unknown")]
    FormatVersion(u16),
    #[error("the board name is empty or not UTF-8 text")]
    BoardName,
    #[error("the image region {start:#010x}-{end:#010x} is empty")]
    EmptyRegion { start: u32, end: u32 },
    #[error("the update mode needs both a product id and a block size")]
    UpdateMode,
}

impl Region {
    #[must_use]
    pub const fn size(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn contains(self, address: u32) -> bool {
        self.start <= address && address < self.end
    }
}

impl Descriptor {
    pub const ALIGNMENT: usize = 64;
    pub const BOARD_NAME_CAPACITY: usize = 32;
    pub const ENCODED_LEN: usize = size_of::<Self>();
    pub const FORMAT_VERSION: u16 = 1;
    pub const MAGIC: [u8; 8] = *b"openwoot";
    pub const MAX_OFFSET: usize = 0x400;

    #[must_use]
    pub const fn new(
        board_name: &str,
        usb: UsbIds,
        image: Region,
        update: Option<UpdateMode>,
    ) -> Self {
        assert!(!board_name.is_empty(), "the board name is empty");
        assert!(
            board_name.len() <= Self::BOARD_NAME_CAPACITY,
            "the board name is too long"
        );
        assert!(image.start < image.end, "the image region is empty");
        let mut name = [0; Self::BOARD_NAME_CAPACITY];
        let (used, _) = name.split_at_mut(board_name.len());
        used.copy_from_slice(board_name.as_bytes());
        let (update_product_id, update_block_size) = match update {
            Some(mode) => {
                assert!(
                    mode.product_id != 0 && mode.block_size != 0,
                    "incomplete update mode"
                );
                (mode.product_id, mode.block_size)
            }
            None => (0, 0),
        };
        Self {
            magic: Self::MAGIC,
            format_version: Self::FORMAT_VERSION,
            usb_vendor_id: usb.vendor_id,
            usb_product_id_base: usb.product_id_base,
            update_product_id,
            image_start: image.start,
            image_end: image.end,
            update_block_size,
            board_name: name,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DescriptorError> {
        let mut fields = Fields(bytes);
        let magic = fields.take()?;
        if magic != Self::MAGIC {
            return Err(DescriptorError::Magic);
        }
        let format_version = u16::from_le_bytes(fields.take()?);
        if format_version != Self::FORMAT_VERSION {
            return Err(DescriptorError::FormatVersion(format_version));
        }
        let descriptor = Self {
            magic,
            format_version,
            usb_vendor_id: u16::from_le_bytes(fields.take()?),
            usb_product_id_base: u16::from_le_bytes(fields.take()?),
            update_product_id: u16::from_le_bytes(fields.take()?),
            image_start: u32::from_le_bytes(fields.take()?),
            image_end: u32::from_le_bytes(fields.take()?),
            update_block_size: u32::from_le_bytes(fields.take()?),
            board_name: fields.take()?,
        };
        if descriptor.board_name().is_none_or(str::is_empty) {
            return Err(DescriptorError::BoardName);
        }
        if descriptor.image_start >= descriptor.image_end {
            return Err(DescriptorError::EmptyRegion {
                start: descriptor.image_start,
                end: descriptor.image_end,
            });
        }
        if (descriptor.update_product_id == 0) != (descriptor.update_block_size == 0) {
            return Err(DescriptorError::UpdateMode);
        }
        Ok(descriptor)
    }

    #[must_use]
    pub fn to_bytes(&self) -> [u8; Self::ENCODED_LEN] {
        let fields: [&[u8]; 9] = [
            &self.magic,
            &self.format_version.to_le_bytes(),
            &self.usb_vendor_id.to_le_bytes(),
            &self.usb_product_id_base.to_le_bytes(),
            &self.update_product_id.to_le_bytes(),
            &self.image_start.to_le_bytes(),
            &self.image_end.to_le_bytes(),
            &self.update_block_size.to_le_bytes(),
            &self.board_name,
        ];
        let mut bytes = [0; Self::ENCODED_LEN];
        let mut rest = bytes.as_mut_slice();
        for field in fields {
            let (target, tail) = rest.split_at_mut(field.len());
            target.copy_from_slice(field);
            rest = tail;
        }
        bytes
    }

    #[must_use]
    pub fn board_name(&self) -> Option<&str> {
        let used = self
            .board_name
            .split(|&byte| byte == 0)
            .next()
            .unwrap_or_default();
        str::from_utf8(used).ok()
    }

    #[must_use]
    pub const fn usb_ids(&self) -> UsbIds {
        UsbIds {
            vendor_id: self.usb_vendor_id,
            product_id_base: self.usb_product_id_base,
        }
    }

    #[must_use]
    pub const fn image(&self) -> Region {
        Region {
            start: self.image_start,
            end: self.image_end,
        }
    }

    #[must_use]
    pub const fn update_mode(&self) -> Option<UpdateMode> {
        if self.update_product_id == 0 {
            None
        } else {
            Some(UpdateMode {
                product_id: self.update_product_id,
                block_size: self.update_block_size,
            })
        }
    }
}

struct Fields<'a>(&'a [u8]);

impl Fields<'_> {
    fn take<const LEN: usize>(&mut self) -> Result<[u8; LEN], DescriptorError> {
        let (field, rest) = self
            .0
            .split_first_chunk::<LEN>()
            .ok_or(DescriptorError::Truncated)?;
        self.0 = rest;
        Ok(*field)
    }
}

#[cfg(test)]
mod tests {
    use core::mem::offset_of;

    use super::*;

    const EXAMPLE: Descriptor = Descriptor::new(
        "Example board",
        UsbIds {
            vendor_id: 0x1234,
            product_id_base: 0x5670,
        },
        Region {
            start: 0x0800_8000,
            end: 0x0801_0000,
        },
        Some(UpdateMode {
            product_id: 0x567F,
            block_size: 256,
        }),
    );

    #[test]
    fn encoding_matches_the_in_memory_layout() {
        assert_eq!(Descriptor::ENCODED_LEN, 60);
        assert_eq!(offset_of!(Descriptor, format_version), 8);
        assert_eq!(offset_of!(Descriptor, usb_vendor_id), 10);
        assert_eq!(offset_of!(Descriptor, usb_product_id_base), 12);
        assert_eq!(offset_of!(Descriptor, update_product_id), 14);
        assert_eq!(offset_of!(Descriptor, image_start), 16);
        assert_eq!(offset_of!(Descriptor, image_end), 20);
        assert_eq!(offset_of!(Descriptor, update_block_size), 24);
        assert_eq!(offset_of!(Descriptor, board_name), 28);
        let bytes = EXAMPLE.to_bytes();
        assert_eq!(&bytes[..8], b"openwoot");
        assert_eq!(&bytes[16..20], &0x0800_8000_u32.to_le_bytes());
        assert_eq!(&bytes[28..41], b"Example board");
    }

    #[test]
    fn survives_a_round_trip() {
        let decoded = Descriptor::from_bytes(&EXAMPLE.to_bytes()).unwrap();
        assert_eq!(decoded, EXAMPLE);
        assert_eq!(decoded.board_name(), Some("Example board"));
        assert_eq!(
            decoded.update_mode(),
            Some(UpdateMode {
                product_id: 0x567F,
                block_size: 256
            })
        );
    }

    #[test]
    fn rejects_an_unknown_format_version() {
        let mut bytes = EXAMPLE.to_bytes();
        bytes[8] = 2;
        assert_eq!(
            Descriptor::from_bytes(&bytes),
            Err(DescriptorError::FormatVersion(2))
        );
    }

    #[test]
    fn rejects_a_half_update_mode() {
        let mut bytes = EXAMPLE.to_bytes();
        bytes[24..28].fill(0);
        assert_eq!(
            Descriptor::from_bytes(&bytes),
            Err(DescriptorError::UpdateMode)
        );
    }

    #[test]
    fn rejects_a_truncated_descriptor() {
        let bytes = EXAMPLE.to_bytes();
        assert_eq!(
            Descriptor::from_bytes(&bytes[..59]),
            Err(DescriptorError::Truncated)
        );
    }
}
