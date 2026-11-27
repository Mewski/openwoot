use heapless::{String, Vec};

use super::{Decode, Encode, Reader, Writer, utf8};

impl<T: Encode + ?Sized> Encode for &T {
    fn encode(&self, writer: &mut Writer<'_>) {
        (**self).encode(writer);
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encode(&self, writer: &mut Writer<'_>) {
        match self {
            None => {
                writer.byte(0);
            }
            Some(value) => {
                writer.byte(1);
                value.encode(writer);
            }
        }
    }
}

impl<T: Decode> Decode for Option<T> {
    fn decode(reader: &mut Reader<'_>) -> Self {
        reader.flag().then(|| T::decode(reader))
    }
}

impl<T: Encode, E: Encode> Encode for Result<T, E> {
    fn encode(&self, writer: &mut Writer<'_>) {
        match self {
            Ok(value) => {
                writer.variant(0);
                value.encode(writer);
            }
            Err(error) => {
                writer.variant(1);
                error.encode(writer);
            }
        }
    }
}

impl<T: Decode, E: Decode> Decode for Result<T, E> {
    fn decode(reader: &mut Reader<'_>) -> Self {
        match reader.variant(2) {
            0 => Ok(T::decode(reader)),
            _ => Err(E::decode(reader)),
        }
    }
}

impl<T: Encode, const N: usize> Encode for [T; N] {
    fn encode(&self, writer: &mut Writer<'_>) {
        for item in self {
            item.encode(writer);
        }
    }
}

impl<T: Decode, const N: usize> Decode for [T; N] {
    fn decode(reader: &mut Reader<'_>) -> Self {
        core::array::from_fn(|_| T::decode(reader))
    }
}

impl<T: Encode, const N: usize> Encode for Vec<T, N> {
    fn encode(&self, writer: &mut Writer<'_>) {
        writer.varint(self.len());
        for item in self {
            item.encode(writer);
        }
    }
}

impl<T: Decode, const N: usize> Decode for Vec<T, N> {
    fn decode(reader: &mut Reader<'_>) -> Self {
        let len = reader.length(N);
        let mut items = Self::new();
        for _ in 0..len {
            if items.push(T::decode(reader)).is_err() {
                reader.fail();
            }
        }
        items
    }
}

impl<const N: usize> Encode for String<N> {
    fn encode(&self, writer: &mut Writer<'_>) {
        writer.text(self);
    }
}

impl<const N: usize> Decode for String<N> {
    fn decode(reader: &mut Reader<'_>) -> Self {
        let mut text = Self::new();
        utf8::read_text(reader, text.as_mut_view());
        text
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
        let mut expected = [0; 64];
        let expected = postcard::to_slice(value, &mut expected).unwrap();
        let mut buffer = [0; 64];
        let len = wire::encode(value, &mut buffer).unwrap();
        assert_eq!(&buffer[..len], expected, "{value:?}");
        assert_eq!(wire::decode::<T>(expected).as_ref(), Some(value));
    }

    fn rejected_like_postcard<T>(bytes: &[u8])
    where
        T: Decode + DeserializeOwned + PartialEq + Debug,
    {
        assert_eq!(wire::decode::<T>(bytes), None, "{bytes:02x?}");
        assert_eq!(postcard::from_bytes::<T>(bytes).ok(), None, "{bytes:02x?}");
    }

    #[test]
    fn containers_encode_as_postcard_does() {
        agrees_with_postcard(&Some(0x1234_u16));
        agrees_with_postcard(&None::<u16>);
        agrees_with_postcard(&Ok::<u8, u32>(7));
        agrees_with_postcard(&Err::<u8, u32>(0x8000));
        agrees_with_postcard(&[1_u16, 300, 0xFFFF]);
        agrees_with_postcard(&Vec::<u16, 4>::from_slice(&[5, 600]).unwrap());
        agrees_with_postcard(&Vec::<u8, 4>::new());
        agrees_with_postcard(&String::<8>::try_from("Wooting").unwrap());
        agrees_with_postcard(&String::<8>::try_from("é").unwrap());
    }

    #[test]
    fn containers_reject_what_postcard_rejects() {
        rejected_like_postcard::<Option<u8>>(&[2, 0]);
        rejected_like_postcard::<Result<u8, u8>>(&[2, 0]);
        rejected_like_postcard::<Vec<u8, 2>>(&[3, 1, 2, 3]);
        rejected_like_postcard::<Vec<u8, 2>>(&[2, 1]);
        rejected_like_postcard::<String<2>>(&[3, b'a', b'b', b'c']);
        rejected_like_postcard::<String<4>>(&[2, 0xC3, 0x28]);
        rejected_like_postcard::<String<4>>(&[3, b'a']);
        rejected_like_postcard::<[u8; 3]>(&[1, 2]);
    }

    #[test]
    fn a_prefix_decodes_and_leaves_the_rest() {
        let bytes = [2, 9, 8, 7, 6];
        assert_eq!(
            wire::take::<Vec<u8, 4>>(&bytes),
            Some((Vec::from_slice(&[9, 8]).unwrap(), &bytes[3..]))
        );
    }
}
