use postcard_rpc::standard_icd::WireError;

use super::{Encode, Writer};

impl Encode for WireError {
    fn encode(&self, writer: &mut Writer<'_>) {
        match self {
            Self::FrameTooLong(too_long) => {
                writer.variant(0);
                too_long.len.encode(writer);
                too_long.max.encode(writer);
            }
            Self::FrameTooShort(too_short) => {
                writer.variant(1);
                too_short.len.encode(writer);
            }
            Self::DeserFailed => writer.variant(2),
            Self::SerFailed => writer.variant(3),
            Self::UnknownKey => writer.variant(4),
            Self::FailedToSpawn => writer.variant(5),
            Self::KeyTooSmall => writer.variant(6),
        }
    }
}

#[cfg(test)]
mod tests {
    use postcard_rpc::standard_icd::{FrameTooLong, FrameTooShort};

    use super::*;
    use crate::wire;

    #[test]
    fn every_wire_error_encodes_as_postcard_does() {
        let errors = [
            WireError::FrameTooLong(FrameTooLong {
                len: 4000,
                max: 2041,
            }),
            WireError::FrameTooShort(FrameTooShort { len: 3 }),
            WireError::DeserFailed,
            WireError::SerFailed,
            WireError::UnknownKey,
            WireError::FailedToSpawn,
            WireError::KeyTooSmall,
        ];
        for error in errors {
            let mut expected = [0; 16];
            let expected = postcard::to_slice(&error, &mut expected).unwrap();
            let mut buffer = [0; 16];
            let len = wire::encode(&error, &mut buffer).unwrap();
            assert_eq!(&buffer[..len], expected, "{error:?}");
        }
    }
}
