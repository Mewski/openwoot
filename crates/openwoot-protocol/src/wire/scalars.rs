use super::{Decode, Encode, Reader, Writer};

impl Encode for () {
    fn encode(&self, writer: &mut Writer<'_>) {
        let _ = writer;
    }
}

impl Decode for () {
    fn decode(reader: &mut Reader<'_>) -> Self {
        let _ = reader;
    }
}

impl Encode for bool {
    fn encode(&self, writer: &mut Writer<'_>) {
        writer.byte(u8::from(*self));
    }
}

impl Decode for bool {
    fn decode(reader: &mut Reader<'_>) -> Self {
        reader.flag()
    }
}

impl Encode for u8 {
    fn encode(&self, writer: &mut Writer<'_>) {
        writer.byte(*self);
    }
}

impl Decode for u8 {
    fn decode(reader: &mut Reader<'_>) -> Self {
        reader.byte()
    }
}

impl Encode for i8 {
    fn encode(&self, writer: &mut Writer<'_>) {
        writer.byte(self.cast_unsigned());
    }
}

impl Decode for i8 {
    fn decode(reader: &mut Reader<'_>) -> Self {
        reader.byte().cast_signed()
    }
}

impl Encode for u16 {
    fn encode(&self, writer: &mut Writer<'_>) {
        writer.varint(usize::from(*self));
    }
}

impl Decode for u16 {
    fn decode(reader: &mut Reader<'_>) -> Self {
        reader.u16()
    }
}

impl Encode for i16 {
    fn encode(&self, writer: &mut Writer<'_>) {
        let zigzag = (self.cast_unsigned() << 1) ^ (self >> 15).cast_unsigned();
        writer.varint(usize::from(zigzag));
    }
}

impl Decode for i16 {
    fn decode(reader: &mut Reader<'_>) -> Self {
        let zigzag = reader.u16();
        (zigzag >> 1).cast_signed() ^ (zigzag & 1).cast_signed().wrapping_neg()
    }
}

impl Encode for u32 {
    fn encode(&self, writer: &mut Writer<'_>) {
        writer.varint(usize::try_from(*self).unwrap_or(usize::MAX));
    }
}

impl Decode for u32 {
    fn decode(reader: &mut Reader<'_>) -> Self {
        reader.u32()
    }
}

impl Encode for usize {
    fn encode(&self, writer: &mut Writer<'_>) {
        writer.varint(*self);
    }
}

impl Decode for usize {
    fn decode(reader: &mut Reader<'_>) -> Self {
        reader.length(Self::MAX)
    }
}

impl Encode for f32 {
    fn encode(&self, writer: &mut Writer<'_>) {
        writer.bytes(&self.to_le_bytes());
    }
}

impl Decode for f32 {
    fn decode(reader: &mut Reader<'_>) -> Self {
        Self::from_le_bytes(reader.array())
    }
}

#[cfg(test)]
mod tests {
    use core::fmt::Debug;

    use serde::{Serialize, de::DeserializeOwned};

    use super::*;
    use crate::wire;

    fn agrees_with_postcard<T>(value: &T)
    where
        T: Encode + Decode + Serialize + DeserializeOwned + PartialEq + Debug,
    {
        let mut expected = [0; 8];
        let expected = postcard::to_slice(value, &mut expected).unwrap();
        let mut buffer = [0; 8];
        let len = wire::encode(value, &mut buffer).unwrap();
        assert_eq!(&buffer[..len], expected, "{value:?}");
        assert_eq!(wire::decode::<T>(expected).as_ref(), Some(value));
    }

    #[test]
    fn scalars_encode_as_postcard_does() {
        for flag in [false, true] {
            agrees_with_postcard(&flag);
        }
        for byte in [0_u8, 0x7F, 0x80, u8::MAX] {
            agrees_with_postcard(&byte);
            agrees_with_postcard(&byte.cast_signed());
        }
        for value in [0_u16, 0x7F, 0x80, 0x3FFF, 0x4000, u16::MAX] {
            agrees_with_postcard(&value);
            agrees_with_postcard(&value.cast_signed());
        }
        for value in [-1_i16, 1, -64, 63, -65, 64, i16::MIN, i16::MAX] {
            agrees_with_postcard(&value);
        }
        for value in [0_u32, 0x7F, 0x80, 0x1F_FFFF, 0x20_0000, u32::MAX] {
            agrees_with_postcard(&value);
        }
        for value in [0.0_f32, -1.5, f32::MAX, f32::INFINITY] {
            agrees_with_postcard(&value);
        }
        agrees_with_postcard(&());
    }

    #[test]
    fn a_flag_other_than_zero_or_one_does_not_decode() {
        assert_eq!(wire::decode::<bool>(&[2]), None);
        assert_eq!(postcard::from_bytes::<bool>(&[2]).ok(), None);
    }

    #[test]
    fn a_float_keeps_its_bits() {
        let quiet_nan = f32::from_bits(0x7FC0_1234);
        let mut buffer = [0; 4];
        let len = wire::encode(&quiet_nan, &mut buffer).unwrap();
        assert_eq!(
            wire::decode::<f32>(&buffer[..len]).map(f32::to_bits),
            Some(0x7FC0_1234)
        );
    }
}
