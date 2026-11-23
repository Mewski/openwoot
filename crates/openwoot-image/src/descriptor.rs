use core::{fmt, mem::size_of};

use crate::{Hex, Region};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UsbIds {
    pub vendor_id: u16,
    pub product_id_base: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateProtocol {
    WootingFactoryBootloader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateMode {
    pub protocol: UpdateProtocol,
    pub product_id: u16,
    pub block_size: u32,
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
    ram_start: u32,
    ram_end: u32,
    update_protocol: u32,
    update_block_size: u32,
    board_name: [u8; Descriptor::BOARD_NAME_CAPACITY],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DescriptorError {
    #[error("the descriptor is truncated")]
    Truncated,
    #[error("the descriptor does not start with its magic bytes")]
    WrongMagic,
    #[error("descriptor format version {0} is unknown")]
    UnknownFormatVersion(u16),
    #[error("the board name is empty or not UTF-8 text")]
    InvalidBoardName,
    #[error("the image region {}-{} is empty", Hex(*.start), Hex(*.end))]
    EmptyRegion { start: u32, end: u32 },
    #[error("the RAM region {}-{} is empty", Hex(*.start), Hex(*.end))]
    EmptyRam { start: u32, end: u32 },
    #[error("the RAM region ends at {}, not on the 8-byte boundary a stack needs", Hex(*.0))]
    MisalignedRamEnd(u32),
    #[error("the update mode needs a protocol, a product id and a block size")]
    IncompleteUpdateMode,
    #[error("update protocol {0} is unknown")]
    UnknownUpdateProtocol(u32),
    #[error(
        "the image region of {size} bytes is not a whole number of update blocks of {block_size} \
         bytes"
    )]
    RegionNotWholeBlocks { size: u32, block_size: u32 },
}

impl UpdateProtocol {
    const fn to_raw(self) -> u32 {
        match self {
            Self::WootingFactoryBootloader => 1,
        }
    }

    const fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            1 => Some(Self::WootingFactoryBootloader),
            _ => None,
        }
    }
}

impl fmt::Display for UpdateProtocol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WootingFactoryBootloader => "Wooting factory bootloader",
        })
    }
}

impl Descriptor {
    pub const ALIGNMENT: usize = 64;
    pub const BOARD_NAME_CAPACITY: usize = 32;
    pub const ENCODED_LEN: usize = size_of::<Self>();
    pub const FORMAT_VERSION: u16 = 1;
    pub const MAGIC: [u8; 8] = *b"openwoot";
    pub const MAX_OFFSET: usize = 0x400;
    pub const STACK_ALIGNMENT: u32 = 8;

    #[must_use]
    pub const fn new(
        board_name: &str,
        usb: UsbIds,
        image: Region,
        ram: Region,
        update: Option<UpdateMode>,
    ) -> Self {
        assert!(!board_name.is_empty(), "the board name is empty");
        assert!(
            board_name.len() <= Self::BOARD_NAME_CAPACITY,
            "the board name is too long"
        );
        assert!(image.start < image.end, "the image region is empty");
        assert!(ram.start < ram.end, "the RAM region is empty");
        assert!(
            ram.end.is_multiple_of(Self::STACK_ALIGNMENT),
            "the RAM region does not end on the 8-byte boundary a stack needs"
        );
        let mut name = [0; Self::BOARD_NAME_CAPACITY];
        let (used, _) = name.split_at_mut(board_name.len());
        used.copy_from_slice(board_name.as_bytes());
        let (update_protocol, update_product_id, update_block_size) = match update {
            Some(mode) => {
                assert!(
                    mode.product_id != 0 && mode.block_size != 0,
                    "the update mode needs both a product id and a block size"
                );
                assert!(
                    image.size().is_multiple_of(mode.block_size),
                    "the image region is not a whole number of update blocks"
                );
                (mode.protocol.to_raw(), mode.product_id, mode.block_size)
            }
            None => (0, 0, 0),
        };
        Self {
            magic: Self::MAGIC,
            format_version: Self::FORMAT_VERSION,
            usb_vendor_id: usb.vendor_id,
            usb_product_id_base: usb.product_id_base,
            update_product_id,
            image_start: image.start,
            image_end: image.end,
            ram_start: ram.start,
            ram_end: ram.end,
            update_protocol,
            update_block_size,
            board_name: name,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DescriptorError> {
        let mut fields = Fields(bytes);
        let magic = fields.take()?;
        if magic != Self::MAGIC {
            return Err(DescriptorError::WrongMagic);
        }
        let format_version = u16::from_le_bytes(fields.take()?);
        if format_version != Self::FORMAT_VERSION {
            return Err(DescriptorError::UnknownFormatVersion(format_version));
        }
        let descriptor = Self {
            magic,
            format_version,
            usb_vendor_id: u16::from_le_bytes(fields.take()?),
            usb_product_id_base: u16::from_le_bytes(fields.take()?),
            update_product_id: u16::from_le_bytes(fields.take()?),
            image_start: u32::from_le_bytes(fields.take()?),
            image_end: u32::from_le_bytes(fields.take()?),
            ram_start: u32::from_le_bytes(fields.take()?),
            ram_end: u32::from_le_bytes(fields.take()?),
            update_protocol: u32::from_le_bytes(fields.take()?),
            update_block_size: u32::from_le_bytes(fields.take()?),
            board_name: fields.take()?,
        };
        if descriptor.board_name().is_none_or(str::is_empty) {
            return Err(DescriptorError::InvalidBoardName);
        }
        if descriptor.image_start >= descriptor.image_end {
            return Err(DescriptorError::EmptyRegion {
                start: descriptor.image_start,
                end: descriptor.image_end,
            });
        }
        if descriptor.ram_start >= descriptor.ram_end {
            return Err(DescriptorError::EmptyRam {
                start: descriptor.ram_start,
                end: descriptor.ram_end,
            });
        }
        if !descriptor.ram_end.is_multiple_of(Self::STACK_ALIGNMENT) {
            return Err(DescriptorError::MisalignedRamEnd(descriptor.ram_end));
        }
        let present = [
            descriptor.update_protocol != 0,
            descriptor.update_product_id != 0,
            descriptor.update_block_size != 0,
        ];
        if present.contains(&true) && present.contains(&false) {
            return Err(DescriptorError::IncompleteUpdateMode);
        }
        if descriptor.update_protocol != 0
            && UpdateProtocol::from_raw(descriptor.update_protocol).is_none()
        {
            return Err(DescriptorError::UnknownUpdateProtocol(
                descriptor.update_protocol,
            ));
        }
        let size = descriptor.image().size();
        if let Some(mode) = descriptor.update_mode()
            && !size.is_multiple_of(mode.block_size)
        {
            return Err(DescriptorError::RegionNotWholeBlocks {
                size,
                block_size: mode.block_size,
            });
        }
        Ok(descriptor)
    }

    #[must_use]
    pub fn to_bytes(&self) -> [u8; Self::ENCODED_LEN] {
        let fields: [&[u8]; 12] = [
            &self.magic,
            &self.format_version.to_le_bytes(),
            &self.usb_vendor_id.to_le_bytes(),
            &self.usb_product_id_base.to_le_bytes(),
            &self.update_product_id.to_le_bytes(),
            &self.image_start.to_le_bytes(),
            &self.image_end.to_le_bytes(),
            &self.ram_start.to_le_bytes(),
            &self.ram_end.to_le_bytes(),
            &self.update_protocol.to_le_bytes(),
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
    pub const fn ram(&self) -> Region {
        Region {
            start: self.ram_start,
            end: self.ram_end,
        }
    }

    #[must_use]
    pub const fn update_mode(&self) -> Option<UpdateMode> {
        match UpdateProtocol::from_raw(self.update_protocol) {
            Some(protocol) if self.update_product_id != 0 && self.update_block_size != 0 => {
                Some(UpdateMode {
                    protocol,
                    product_id: self.update_product_id,
                    block_size: self.update_block_size,
                })
            }
            _ => None,
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

    const PROTOCOL: usize = 32;

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
        Region {
            start: 0x2000_0000,
            end: 0x2003_0000,
        },
        Some(UpdateMode {
            protocol: UpdateProtocol::WootingFactoryBootloader,
            product_id: 0x567F,
            block_size: 256,
        }),
    );

    #[test]
    fn encoding_matches_the_in_memory_layout() {
        assert_eq!(Descriptor::ENCODED_LEN, 72);
        assert_eq!(offset_of!(Descriptor, format_version), 8);
        assert_eq!(offset_of!(Descriptor, usb_vendor_id), 10);
        assert_eq!(offset_of!(Descriptor, usb_product_id_base), 12);
        assert_eq!(offset_of!(Descriptor, update_product_id), 14);
        assert_eq!(offset_of!(Descriptor, image_start), 16);
        assert_eq!(offset_of!(Descriptor, image_end), 20);
        assert_eq!(offset_of!(Descriptor, ram_start), 24);
        assert_eq!(offset_of!(Descriptor, ram_end), 28);
        assert_eq!(offset_of!(Descriptor, update_protocol), PROTOCOL);
        assert_eq!(offset_of!(Descriptor, update_block_size), 36);
        assert_eq!(offset_of!(Descriptor, board_name), 40);
        let bytes = EXAMPLE.to_bytes();
        assert_eq!(&bytes[..8], b"openwoot");
        assert_eq!(&bytes[8..10], &1_u16.to_le_bytes());
        assert_eq!(&bytes[16..20], &0x0800_8000_u32.to_le_bytes());
        assert_eq!(&bytes[24..28], &0x2000_0000_u32.to_le_bytes());
        assert_eq!(&bytes[28..32], &0x2003_0000_u32.to_le_bytes());
        assert_eq!(&bytes[PROTOCOL..PROTOCOL + 4], &1_u32.to_le_bytes());
        assert_eq!(&bytes[40..53], b"Example board");
    }

    #[test]
    fn survives_a_round_trip() {
        let decoded = Descriptor::from_bytes(&EXAMPLE.to_bytes()).unwrap();
        assert_eq!(decoded, EXAMPLE);
        assert_eq!(decoded.board_name(), Some("Example board"));
        assert_eq!(
            decoded.image(),
            Region {
                start: 0x0800_8000,
                end: 0x0801_0000
            }
        );
        assert_eq!(
            decoded.ram(),
            Region {
                start: 0x2000_0000,
                end: 0x2003_0000
            }
        );
        assert_eq!(
            decoded.update_mode(),
            Some(UpdateMode {
                protocol: UpdateProtocol::WootingFactoryBootloader,
                product_id: 0x567F,
                block_size: 256
            })
        );
    }

    #[test]
    fn survives_a_round_trip_without_update_mode() {
        let descriptor = Descriptor::new(
            "Example board",
            EXAMPLE.usb_ids(),
            EXAMPLE.image(),
            EXAMPLE.ram(),
            None,
        );
        let bytes = descriptor.to_bytes();
        assert!(bytes[PROTOCOL..PROTOCOL + 8].iter().all(|&byte| byte == 0));
        let decoded = Descriptor::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, descriptor);
        assert_eq!(decoded.update_mode(), None);
        assert_eq!(decoded.ram(), EXAMPLE.ram());
    }

    #[test]
    fn rejects_an_unknown_format_version() {
        let mut bytes = EXAMPLE.to_bytes();
        bytes[8] = 2;
        assert_eq!(
            Descriptor::from_bytes(&bytes),
            Err(DescriptorError::UnknownFormatVersion(2))
        );
    }

    #[test]
    fn rejects_an_unknown_update_protocol() {
        for raw in [2, 0x100, u32::MAX] {
            let mut bytes = EXAMPLE.to_bytes();
            bytes[PROTOCOL..PROTOCOL + 4].copy_from_slice(&raw.to_le_bytes());
            assert_eq!(
                Descriptor::from_bytes(&bytes),
                Err(DescriptorError::UnknownUpdateProtocol(raw))
            );
        }
    }

    #[test]
    fn rejects_a_partial_update_mode() {
        for field in [14..16, PROTOCOL..PROTOCOL + 4, 36..40] {
            let mut bytes = EXAMPLE.to_bytes();
            bytes[field.clone()].fill(0);
            assert_eq!(
                Descriptor::from_bytes(&bytes),
                Err(DescriptorError::IncompleteUpdateMode),
                "{field:?}"
            );
        }
        let mut bytes = EXAMPLE.to_bytes();
        bytes[14..16].fill(0);
        bytes[36..40].fill(0);
        bytes[PROTOCOL..PROTOCOL + 4].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            Descriptor::from_bytes(&bytes),
            Err(DescriptorError::IncompleteUpdateMode)
        );
    }

    #[test]
    fn rejects_a_region_that_is_not_whole_update_blocks() {
        let mut bytes = EXAMPLE.to_bytes();
        bytes[20..24].copy_from_slice(&0x0800_8480_u32.to_le_bytes());
        assert_eq!(
            Descriptor::from_bytes(&bytes),
            Err(DescriptorError::RegionNotWholeBlocks {
                size: 0x480,
                block_size: 256
            })
        );
        bytes[14..16].fill(0);
        bytes[PROTOCOL..PROTOCOL + 4].fill(0);
        bytes[36..40].fill(0);
        assert_eq!(
            Descriptor::from_bytes(&bytes).map(|descriptor| descriptor.update_mode()),
            Ok(None)
        );
    }

    #[test]
    fn rejects_an_empty_ram_region_and_one_that_ends_off_the_stack_alignment() {
        let mut bytes = EXAMPLE.to_bytes();
        bytes[28..32].copy_from_slice(&0x2000_0000_u32.to_le_bytes());
        assert_eq!(
            Descriptor::from_bytes(&bytes),
            Err(DescriptorError::EmptyRam {
                start: 0x2000_0000,
                end: 0x2000_0000
            })
        );
        bytes[28..32].copy_from_slice(&0x2002_FFFC_u32.to_le_bytes());
        assert_eq!(
            Descriptor::from_bytes(&bytes),
            Err(DescriptorError::MisalignedRamEnd(0x2002_FFFC))
        );
    }

    #[test]
    fn rejects_an_empty_region_a_wrong_magic_and_a_name_that_is_not_text() {
        let mut bytes = EXAMPLE.to_bytes();
        bytes[20..24].copy_from_slice(&0x0800_8000_u32.to_le_bytes());
        assert_eq!(
            Descriptor::from_bytes(&bytes),
            Err(DescriptorError::EmptyRegion {
                start: 0x0800_8000,
                end: 0x0800_8000
            })
        );
        let mut bytes = EXAMPLE.to_bytes();
        bytes[0] = b'O';
        assert_eq!(
            Descriptor::from_bytes(&bytes),
            Err(DescriptorError::WrongMagic)
        );
        let mut bytes = EXAMPLE.to_bytes();
        bytes[40] = 0xFF;
        assert_eq!(
            Descriptor::from_bytes(&bytes),
            Err(DescriptorError::InvalidBoardName)
        );
        bytes[40] = 0;
        assert_eq!(
            Descriptor::from_bytes(&bytes),
            Err(DescriptorError::InvalidBoardName)
        );
    }

    #[test]
    fn rejects_a_truncated_descriptor() {
        let bytes = EXAMPLE.to_bytes();
        assert_eq!(
            Descriptor::from_bytes(&bytes[..Descriptor::ENCODED_LEN - 1]),
            Err(DescriptorError::Truncated)
        );
    }
}
