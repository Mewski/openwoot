const WIRE_VARINT: u64 = 0;
const WIRE_FIXED64: u64 = 1;
const WIRE_BYTES: u64 = 2;
const WIRE_FIXED32: u64 = 5;
const VARINT_MAX_BYTES: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Wire<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Fixed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Malformed;

#[derive(Clone, Debug)]
pub(super) struct Fields<'a> {
    rest: &'a [u8],
}

impl<'a> Fields<'a> {
    pub(super) const fn new(message: &'a [u8]) -> Self {
        Self { rest: message }
    }

    fn varint(&mut self) -> Result<u64, Malformed> {
        let mut value = 0u64;
        for index in 0..VARINT_MAX_BYTES {
            let (&byte, rest) = self.rest.split_first().ok_or(Malformed)?;
            self.rest = rest;
            value |= u64::from(byte & 0x7F) << (7 * index);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(Malformed)
    }

    fn bytes(&mut self, len: usize) -> Result<&'a [u8], Malformed> {
        let (taken, rest) = self.rest.split_at_checked(len).ok_or(Malformed)?;
        self.rest = rest;
        Ok(taken)
    }

    fn field(&mut self) -> Result<(u32, Wire<'a>), Malformed> {
        let tag = self.varint()?;
        let number = u32::try_from(tag >> 3).map_err(|_| Malformed)?;
        if number == 0 {
            return Err(Malformed);
        }
        let wire = match tag & 0b111 {
            WIRE_VARINT => Wire::Varint(self.varint()?),
            WIRE_BYTES => {
                let len = usize::try_from(self.varint()?).map_err(|_| Malformed)?;
                Wire::Bytes(self.bytes(len)?)
            }
            WIRE_FIXED64 => {
                self.bytes(8)?;
                Wire::Fixed
            }
            WIRE_FIXED32 => {
                self.bytes(4)?;
                Wire::Fixed
            }
            _ => return Err(Malformed),
        };
        Ok((number, wire))
    }
}

impl<'a> Iterator for Fields<'a> {
    type Item = Result<(u32, Wire<'a>), Malformed>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest.is_empty() {
            return None;
        }
        let field = self.field();
        if field.is_err() {
            self.rest = &[];
        }
        Some(field)
    }
}

pub(super) fn is_well_formed(message: &[u8]) -> bool {
    Fields::new(message).all(|field| field.is_ok())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    #[test]
    fn fields_of_every_wire_type_are_read_in_order() {
        let message = [
            0x08, 0x96, 0x01, 0x12, 0x02, 0xAA, 0xBB, 0x1D, 1, 2, 3, 4, 0x21, 0, 0, 0, 0, 0, 0, 0,
            0,
        ];
        let fields: Vec<_> = Fields::new(&message).collect();
        assert_eq!(
            fields,
            [
                Ok((1, Wire::Varint(150))),
                Ok((2, Wire::Bytes(&[0xAA, 0xBB]))),
                Ok((3, Wire::Fixed)),
                Ok((4, Wire::Fixed)),
            ]
        );
    }

    #[test]
    fn truncated_or_grouped_fields_are_malformed() {
        assert!(!is_well_formed(&[0x12, 0x05, 0x01]));
        assert!(!is_well_formed(&[0x08]));
        assert!(!is_well_formed(&[0x0B]));
        assert!(!is_well_formed(&[0x00, 0x01]));
        assert!(!is_well_formed(&[
            0x08, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x01
        ]));
        assert!(is_well_formed(&[]));
    }
}
