use super::protobuf::{Fields, Malformed, Wire, is_well_formed};

const CAPACITY: usize = 58;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuxRecord {
    bytes: [u8; CAPACITY],
    len: usize,
}

impl AuxRecord {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or_default()
    }

    pub(super) fn from_message(message: &[u8]) -> Result<Self, Malformed> {
        let has_first = Fields::new(message).any(|field| matches!(field, Ok((1, Wire::Bytes(_)))));
        if !has_first || !is_well_formed(message) {
            return Err(Malformed);
        }
        let mut bytes = [0; CAPACITY];
        bytes
            .get_mut(..message.len())
            .ok_or(Malformed)?
            .copy_from_slice(message);
        Ok(Self {
            bytes,
            len: message.len(),
        })
    }
}
