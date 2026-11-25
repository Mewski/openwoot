use sequential_storage::map::{self, SerializationError, Value};
use serde::{Serialize, de::DeserializeOwned};

pub(super) const KEY_LEN: usize = 2;
pub(super) const VERSION_LEN: usize = 1;
pub(super) const MAX_VALUE_LEN: usize = 2048;

const GLOBAL_SLOT: u8 = 0xFF;
const LINKED_SLOTS: u8 = 0x80;

pub trait Record: Serialize + DeserializeOwned {
    const ITEM: u8;
    const VERSION: u8;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Global,
    Profile(u8),
    Linked(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RecordKey(u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stored<'a> {
    pub version: u8,
    pub body: &'a [u8],
}

pub(super) struct Encoding<'r, R>(pub(super) &'r R);

pub(super) struct Encoder<F> {
    pub(super) version: u8,
    pub(super) encode: F,
}

impl RecordKey {
    #[must_use]
    pub const fn new(scope: Scope, item: u8) -> Self {
        let slot = match scope {
            Scope::Global => GLOBAL_SLOT,
            Scope::Profile(index) => index,
            Scope::Linked(index) => LINKED_SLOTS | index,
        };
        Self(u16::from_be_bytes([slot, item]))
    }

    #[must_use]
    pub const fn of<R: Record>(scope: Scope) -> Self {
        Self::new(scope, R::ITEM)
    }

    #[must_use]
    pub const fn scope(self) -> Scope {
        match self.0.to_be_bytes() {
            [GLOBAL_SLOT, _] => Scope::Global,
            [slot, _] if slot & LINKED_SLOTS != 0 => Scope::Linked(slot & !LINKED_SLOTS),
            [index, _] => Scope::Profile(index),
        }
    }

    #[must_use]
    pub const fn item(self) -> u8 {
        let [_, item] = self.0.to_be_bytes();
        item
    }
}

impl Stored<'_> {
    #[must_use]
    pub fn decode<R: Record>(&self) -> Option<R> {
        if self.version == R::VERSION {
            postcard::from_bytes(self.body).ok()
        } else {
            None
        }
    }
}

impl map::Key for RecordKey {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
        <u16 as map::Key>::serialize_into(&self.0, buffer)
    }

    fn deserialize_from(buffer: &[u8]) -> Result<(Self, usize), SerializationError> {
        <u16 as map::Key>::deserialize_from(buffer).map(|(key, len)| (Self(key), len))
    }

    fn get_len(buffer: &[u8]) -> Result<usize, SerializationError> {
        <u16 as map::Key>::get_len(buffer)
    }
}

impl<'a> Value<'a> for Stored<'a> {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
        let (version, body) = split(buffer)?;
        *version = self.version;
        body.get_mut(..self.body.len())
            .ok_or(SerializationError::BufferTooSmall)?
            .copy_from_slice(self.body);
        Ok(VERSION_LEN + self.body.len())
    }

    fn deserialize_from(buffer: &'a [u8]) -> Result<(Self, usize), SerializationError> {
        let (&version, body) = buffer
            .split_first()
            .ok_or(SerializationError::InvalidFormat)?;
        Ok((Self { version, body }, buffer.len()))
    }
}

impl<'a, R: Record> Value<'a> for Encoding<'_, R> {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
        let (version, body) = split(buffer)?;
        *version = R::VERSION;
        let body = postcard::to_slice(self.0, body).map_err(|error| match error {
            postcard::Error::SerializeBufferFull => SerializationError::BufferTooSmall,
            _ => SerializationError::InvalidData,
        })?;
        Ok(VERSION_LEN + body.len())
    }

    fn deserialize_from(_buffer: &'a [u8]) -> Result<(Self, usize), SerializationError> {
        Err(SerializationError::InvalidData)
    }
}

impl<'a, F: Fn(&mut [u8]) -> Option<usize>> Value<'a> for Encoder<F> {
    fn serialize_into(&self, buffer: &mut [u8]) -> Result<usize, SerializationError> {
        let (version, body) = split(buffer)?;
        *version = self.version;
        let capacity = body.len();
        match (self.encode)(body) {
            Some(len) if len <= capacity => Ok(VERSION_LEN + len),
            Some(_) => Err(SerializationError::BufferTooSmall),
            None => Err(SerializationError::InvalidData),
        }
    }

    fn deserialize_from(_buffer: &'a [u8]) -> Result<(Self, usize), SerializationError> {
        Err(SerializationError::InvalidData)
    }
}

fn split(buffer: &mut [u8]) -> Result<(&mut u8, &mut [u8]), SerializationError> {
    let (version, body) = buffer
        .split_first_mut()
        .ok_or(SerializationError::BufferTooSmall)?;
    let capacity = body.len().min(MAX_VALUE_LEN);
    Ok((version, body.get_mut(..capacity).unwrap_or_default()))
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Brightness(u8);

    impl Record for Brightness {
        const ITEM: u8 = 7;
        const VERSION: u8 = 2;
    }

    #[test]
    fn record_keys_hold_the_slot_in_the_high_byte_and_the_item_in_the_low_byte() {
        let global = RecordKey::of::<Brightness>(Scope::Global);
        assert_eq!(global, RecordKey(0xFF07));
        assert_eq!((global.scope(), global.item()), (Scope::Global, 7));
        let profile = RecordKey::new(Scope::Profile(3), 0x21);
        assert_eq!(profile, RecordKey(0x0321));
        assert_eq!(profile.scope(), Scope::Profile(3));
    }

    #[test]
    fn values_carry_their_version_in_front_of_the_body() {
        let mut buffer = [0; 8];
        let len = Encoding(&Brightness(200))
            .serialize_into(&mut buffer)
            .unwrap();
        assert_eq!(buffer[..len], [2, 200]);
        let (stored, _) = Stored::deserialize_from(&buffer[..len]).unwrap();
        assert_eq!(
            stored,
            Stored {
                version: 2,
                body: &[200]
            }
        );
        assert_eq!(stored.decode::<Brightness>(), Some(Brightness(200)));
        assert_eq!(
            Encoding(&Brightness(1)).serialize_into(&mut [0; 1]),
            Err(SerializationError::BufferTooSmall)
        );
    }

    #[test]
    fn a_body_that_does_not_decode_counts_as_absent() {
        let broken = Stored {
            version: 2,
            body: &[],
        };
        assert_eq!(broken.decode::<Brightness>(), None);
    }

    #[test]
    fn an_encoder_writes_the_version_and_the_body_it_was_given() {
        let mut buffer = [0; 8];
        let encoder = Encoder {
            version: 3,
            encode: |body: &mut [u8]| {
                postcard::to_slice(&Brightness(9), body)
                    .ok()
                    .map(|body| body.len())
            },
        };
        let len = encoder.serialize_into(&mut buffer).unwrap();
        assert_eq!(buffer[..len], [3, 9]);
    }

    #[test]
    fn the_body_offered_to_an_encoder_ends_at_the_largest_value() {
        let mut buffer = [0; 2 * MAX_VALUE_LEN];
        let whole = Encoder {
            version: 1,
            encode: |body: &mut [u8]| Some(body.len()),
        };
        assert_eq!(
            whole.serialize_into(&mut buffer),
            Ok(VERSION_LEN + MAX_VALUE_LEN)
        );
    }

    #[test]
    fn an_encoder_that_fails_or_claims_too_much_stores_nothing() {
        let mut buffer = [0; 8];
        let failing = Encoder {
            version: 1,
            encode: |_: &mut [u8]| None,
        };
        assert_eq!(
            failing.serialize_into(&mut buffer),
            Err(SerializationError::InvalidData)
        );
        let oversized = Encoder {
            version: 1,
            encode: |body: &mut [u8]| Some(body.len() + 1),
        };
        assert_eq!(
            oversized.serialize_into(&mut buffer),
            Err(SerializationError::BufferTooSmall)
        );
        assert!(
            Encoder::<fn(&mut [u8]) -> Option<usize>> {
                version: 1,
                encode: |_| Some(0),
            }
            .serialize_into(&mut [])
            .is_err()
        );
    }
}
