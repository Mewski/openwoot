use crate::{Descriptor, DescriptorError, Hex, Region};

const VECTOR_SIZE: usize = 4;
const RESET_VECTOR_INDEX: usize = 1;
const STACK_REGION: Region = Region {
    start: 0x2000_0000,
    end: 0x4000_0001,
};
const STACK_ALIGNMENT: u32 = 8;
const ERASED_WORD: u32 = 0xFFFF_FFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Image<'a> {
    bytes: &'a [u8],
    descriptor: Descriptor,
    descriptor_offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("the image has {0} bytes, too few for a vector table")]
    Truncated(usize),
    #[error("the image has no descriptor behind its vector table")]
    MissingDescriptor,
    #[error("the image descriptor is invalid")]
    InvalidDescriptor(#[from] DescriptorError),
    #[error("the image starts at {}, its descriptor at {}", Hex(*.found), Hex(*.expected))]
    StartMismatch { found: u32, expected: u32 },
    #[error(
        "the image has {size} bytes; its area {}-{} holds {capacity} bytes",
        Hex(*.start),
        Hex(*.end)
    )]
    TooLarge {
        size: usize,
        start: u32,
        end: u32,
        capacity: u32,
    },
    #[error("initial stack pointer {} is not an aligned RAM address", Hex(*.0))]
    MisplacedStackPointer(u32),
    #[error("vector {index} ({}) does not point into the image as Thumb code", Hex(*.address))]
    VectorOutsideImage { index: usize, address: u32 },
    #[error("reset vector {} does not point behind the descriptor as Thumb code", Hex(*.0))]
    ResetVectorOutsideCode(u32),
}

impl<'a> Image<'a> {
    pub fn new(bytes: &'a [u8], start: Option<u32>) -> Result<Self, Error> {
        if bytes.len() < Descriptor::ALIGNMENT {
            return Err(Error::Truncated(bytes.len()));
        }
        let (descriptor_offset, descriptor) = locate_descriptor(bytes)?;
        let region = descriptor.image();
        if let Some(found) = start.filter(|&found| found != region.start) {
            return Err(Error::StartMismatch {
                found,
                expected: region.start,
            });
        }
        let size = u32::try_from(bytes.len())
            .ok()
            .filter(|&size| size <= region.size())
            .ok_or(Error::TooLarge {
                size: bytes.len(),
                start: region.start,
                end: region.end,
                capacity: region.size(),
            })?;
        let image = Self {
            bytes,
            descriptor,
            descriptor_offset,
        };
        let content = Region {
            start: region.start,
            end: region.start + size,
        };

        let stack_pointer = image.vector(0);
        if !STACK_REGION.contains(stack_pointer) || !stack_pointer.is_multiple_of(STACK_ALIGNMENT) {
            return Err(Error::MisplacedStackPointer(stack_pointer));
        }
        for index in RESET_VECTOR_INDEX + 1..descriptor_offset / VECTOR_SIZE {
            let address = image.vector(index);
            let unused = address == 0 || address == ERASED_WORD;
            if !unused && !is_thumb_code_in(content, address) {
                return Err(Error::VectorOutsideImage { index, address });
            }
        }
        let reset = image.vector(RESET_VECTOR_INDEX);
        let code = Region {
            start: image.descriptor_end(),
            end: content.end,
        };
        if !is_thumb_code_in(code, reset) {
            return Err(Error::ResetVectorOutsideCode(reset));
        }
        Ok(image)
    }

    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[must_use]
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }

    #[must_use]
    pub const fn descriptor_offset(&self) -> usize {
        self.descriptor_offset
    }

    fn descriptor_end(&self) -> u32 {
        let end = self.descriptor_offset + Descriptor::ENCODED_LEN;
        self.descriptor.image().start + u32::try_from(end).unwrap_or(u32::MAX)
    }

    fn vector(&self, index: usize) -> u32 {
        self.bytes
            .get(index * VECTOR_SIZE..)
            .and_then(<[u8]>::first_chunk)
            .map_or(0, |bytes| u32::from_le_bytes(*bytes))
    }
}

fn locate_descriptor(bytes: &[u8]) -> Result<(usize, Descriptor), Error> {
    let offset = (Descriptor::ALIGNMENT..=Descriptor::MAX_OFFSET)
        .step_by(Descriptor::ALIGNMENT)
        .find(|&offset| {
            bytes
                .get(offset..)
                .is_some_and(|rest| rest.starts_with(&Descriptor::MAGIC))
        })
        .ok_or(Error::MissingDescriptor)?;
    let descriptor = Descriptor::from_bytes(bytes.get(offset..).unwrap_or_default())?;
    Ok((offset, descriptor))
}

const fn is_thumb_code_in(region: Region, address: u32) -> bool {
    address & 1 == 1 && region.contains(address & !1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UpdateMode, UsbIds};

    const START: u32 = 0x0800_8000;
    const DESCRIPTOR_OFFSET: usize = 0x1C0;
    const CODE: u32 = START + 0x200;

    const DESCRIPTOR: Descriptor = Descriptor::new(
        "Example board",
        UsbIds {
            vendor_id: 0x1234,
            product_id_base: 0x5670,
        },
        Region {
            start: START,
            end: START + 0x1_0000,
        },
        Some(UpdateMode {
            product_id: 0x567F,
            block_size: 256,
        }),
    );

    fn firmware(size: usize) -> Vec<u8> {
        let mut bytes = vec![0xFF; size];
        let mut vectors = vec![0x2003_0000, CODE + 1];
        vectors.extend([CODE + 3; 105]);
        for (index, vector) in vectors.iter().enumerate() {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&vector.to_le_bytes());
        }
        bytes[DESCRIPTOR_OFFSET..DESCRIPTOR_OFFSET + Descriptor::ENCODED_LEN]
            .copy_from_slice(&DESCRIPTOR.to_bytes());
        bytes
    }

    fn set_vector(bytes: &mut [u8], index: usize, value: u32) {
        bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn accepts_a_well_formed_image() {
        let bytes = firmware(0x400);
        let image = Image::new(&bytes, Some(START)).unwrap();
        assert_eq!(image.descriptor_offset(), DESCRIPTOR_OFFSET);
        assert_eq!(image.descriptor().board_name(), Some("Example board"));
    }

    #[test]
    fn rejects_a_wrong_start_address() {
        let bytes = firmware(0x400);
        assert_eq!(
            Image::new(&bytes, Some(0x0800_0000)),
            Err(Error::StartMismatch {
                found: 0x0800_0000,
                expected: START
            })
        );
    }

    #[test]
    fn rejects_an_image_past_its_end() {
        let bytes = firmware(0x1_0001);
        assert!(matches!(
            Image::new(&bytes, None),
            Err(Error::TooLarge { size: 0x1_0001, .. })
        ));
    }

    #[test]
    fn rejects_a_reset_vector_outside_the_image() {
        let mut bytes = firmware(0x400);
        set_vector(&mut bytes, 1, START + 0x401);
        assert_eq!(
            Image::new(&bytes, None),
            Err(Error::ResetVectorOutsideCode(START + 0x401))
        );
    }

    #[test]
    fn rejects_a_reset_vector_into_the_descriptor() {
        let mut bytes = firmware(0x400);
        set_vector(&mut bytes, 1, START + 0x1C1);
        assert_eq!(
            Image::new(&bytes, None),
            Err(Error::ResetVectorOutsideCode(START + 0x1C1))
        );
    }

    #[test]
    fn rejects_an_even_reset_vector() {
        let mut bytes = firmware(0x400);
        set_vector(&mut bytes, 1, CODE);
        assert_eq!(
            Image::new(&bytes, None),
            Err(Error::ResetVectorOutsideCode(CODE))
        );
    }

    #[test]
    fn rejects_a_handler_outside_the_image() {
        let mut bytes = firmware(0x400);
        set_vector(&mut bytes, 20, 0x0800_0001);
        assert_eq!(
            Image::new(&bytes, None),
            Err(Error::VectorOutsideImage {
                index: 20,
                address: 0x0800_0001
            })
        );
    }

    #[test]
    fn rejects_a_stack_pointer_outside_ram() {
        let mut bytes = firmware(0x400);
        set_vector(&mut bytes, 0, 0x0803_0000);
        assert_eq!(
            Image::new(&bytes, None),
            Err(Error::MisplacedStackPointer(0x0803_0000))
        );
    }

    #[test]
    fn requires_a_descriptor() {
        let mut bytes = firmware(0x400);
        bytes[DESCRIPTOR_OFFSET] = 0;
        assert_eq!(Image::new(&bytes, None), Err(Error::MissingDescriptor));
    }
}
