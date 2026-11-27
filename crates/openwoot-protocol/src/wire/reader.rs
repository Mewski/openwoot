const CONTINUATION: u8 = 0x80;
const PAYLOAD: u8 = 0x7F;
const LENGTH_BYTES: u32 = usize::BITS.div_ceil(7);
const LENGTH_LAST_BYTE: u8 = (1 << (usize::BITS % 7)) - 1;

pub struct Reader<'a> {
    rest: &'a [u8],
    failed: bool,
}

impl<'a> Reader<'a> {
    #[inline(never)]
    pub fn variant(&mut self, count: u32) -> u32 {
        let index = self.u32();
        if index < count {
            index
        } else {
            self.fail();
            0
        }
    }

    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self {
            rest: bytes,
            failed: false,
        }
    }

    pub(crate) fn finish(self) -> Option<&'a [u8]> {
        (!self.failed).then_some(self.rest)
    }

    pub(crate) const fn fail(&mut self) {
        self.failed = true;
    }

    #[inline(never)]
    pub(crate) fn byte(&mut self) -> u8 {
        if let Some((&byte, rest)) = self.rest.split_first() {
            self.rest = rest;
            byte
        } else {
            self.fail();
            0
        }
    }

    #[inline(never)]
    pub(crate) fn bytes(&mut self, len: usize) -> &'a [u8] {
        if let Some((taken, rest)) = self.rest.split_at_checked(len) {
            self.rest = rest;
            taken
        } else {
            self.fail();
            &[]
        }
    }

    #[inline(never)]
    pub(crate) fn array<const N: usize>(&mut self) -> [u8; N] {
        if let Some((taken, rest)) = self.rest.split_first_chunk() {
            self.rest = rest;
            *taken
        } else {
            self.fail();
            [0; N]
        }
    }

    #[inline(never)]
    pub(crate) fn flag(&mut self) -> bool {
        match self.byte() {
            0 => false,
            1 => true,
            _ => {
                self.fail();
                false
            }
        }
    }

    #[inline(never)]
    pub(crate) fn u16(&mut self) -> u16 {
        u16::try_from(self.varint(3, 0b11)).unwrap_or_default()
    }

    #[inline(never)]
    pub(crate) fn u32(&mut self) -> u32 {
        u32::try_from(self.varint(5, 0b1111)).unwrap_or_default()
    }

    #[inline(never)]
    pub(crate) fn length(&mut self, capacity: usize) -> usize {
        let len = self.varint(LENGTH_BYTES, LENGTH_LAST_BYTE);
        if len <= capacity {
            len
        } else {
            self.fail();
            0
        }
    }

    fn varint(&mut self, max_bytes: u32, last_byte_max: u8) -> usize {
        let mut value = 0;
        for position in 0..max_bytes {
            let byte = self.byte();
            value |= usize::from(byte & PAYLOAD).wrapping_shl(7 * position);
            if byte & CONTINUATION == 0 {
                if position + 1 == max_bytes && byte > last_byte_max {
                    self.fail();
                }
                return value;
            }
        }
        self.fail();
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read<'a, T>(bytes: &'a [u8], read: impl FnOnce(&mut Reader<'a>) -> T) -> Option<(T, usize)> {
        let mut reader = Reader::new(bytes);
        let value = read(&mut reader);
        reader.finish().map(|rest| (value, rest.len()))
    }

    #[test]
    fn varints_accept_what_postcard_accepts_and_nothing_else() {
        let cases: [&[u8]; 14] = [
            &[0x00],
            &[0x7F],
            &[0x80, 0x01],
            &[0x80, 0x00],
            &[0xFF, 0xFF, 0x03],
            &[0xFF, 0xFF, 0x04],
            &[0x80, 0x80, 0x00],
            &[0x80, 0x80, 0x80, 0x00],
            &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F],
            &[0xFF, 0xFF, 0xFF, 0xFF, 0x10],
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x00],
            &[0x80],
            &[],
            &[0x81, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01],
        ];
        for bytes in cases {
            assert_eq!(
                read(bytes, Reader::u16),
                postcard::take_from_bytes::<u16>(bytes)
                    .ok()
                    .map(|(value, rest)| (value, rest.len())),
                "u16 from {bytes:02x?}"
            );
            assert_eq!(
                read(bytes, Reader::u32),
                postcard::take_from_bytes::<u32>(bytes)
                    .ok()
                    .map(|(value, rest)| (value, rest.len())),
                "u32 from {bytes:02x?}"
            );
            assert_eq!(
                read(bytes, |reader| reader.length(usize::MAX)),
                postcard::take_from_bytes::<usize>(bytes)
                    .ok()
                    .map(|(value, rest)| (value, rest.len())),
                "length from {bytes:02x?}"
            );
        }
    }

    #[test]
    fn flags_are_zero_or_one() {
        assert_eq!(read(&[0], Reader::flag), Some((false, 0)));
        assert_eq!(read(&[1, 9], Reader::flag), Some((true, 1)));
        assert_eq!(read(&[2], Reader::flag), None);
        assert_eq!(read(&[], Reader::flag), None);
    }

    #[test]
    fn indices_and_lengths_stay_below_their_bound() {
        assert_eq!(read(&[2], |reader| reader.variant(3)), Some((2, 0)));
        assert_eq!(read(&[3], |reader| reader.variant(3)), None);
        assert_eq!(read(&[4], |reader| reader.length(4)), Some((4, 0)));
        assert_eq!(read(&[5], |reader| reader.length(4)), None);
    }

    #[test]
    fn a_failed_reader_reads_zeros_without_ending_early() {
        let mut reader = Reader::new(&[1, 2]);
        assert_eq!(reader.bytes(3), &[] as &[u8]);
        assert_eq!(reader.byte(), 1);
        assert_eq!(reader.array::<2>(), [0, 0]);
        assert_eq!(reader.finish(), None);
    }
}
