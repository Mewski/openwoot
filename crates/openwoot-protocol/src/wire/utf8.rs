use heapless::string::StringView;

use super::Reader;

const CONTINUATION_MASK: u8 = 0xC0;
const CONTINUATION_TAG: u8 = 0x80;
const CONTINUATION_PAYLOAD: u8 = 0x3F;

#[inline(never)]
pub(super) fn read_text(reader: &mut Reader<'_>, text: &mut StringView) {
    let len = reader.length(usize::MAX);
    let bytes = reader.bytes(len);
    if !push_scalars(bytes, text) {
        reader.fail();
    }
}

fn push_scalars(mut bytes: &[u8], text: &mut StringView) -> bool {
    while let Some((&lead, rest)) = bytes.split_first() {
        let (continuations, least) = match lead {
            0x00..=0x7F => (0, 0),
            0xC0..=0xDF => (1, 0x80),
            0xE0..=0xEF => (2, 0x800),
            0xF0..=0xF7 => (3, 0x1_0000),
            _ => return false,
        };
        let Some((tail, after)) = rest.split_at_checked(continuations) else {
            return false;
        };
        let mut scalar = u32::from(lead & (0x7F >> continuations));
        for &byte in tail {
            if byte & CONTINUATION_MASK != CONTINUATION_TAG {
                return false;
            }
            scalar = scalar << 6 | u32::from(byte & CONTINUATION_PAYLOAD);
        }
        match char::from_u32(scalar) {
            Some(character) if scalar >= least && text.push(character).is_ok() => bytes = after,
            _ => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use heapless::String;

    use super::*;

    fn agrees_with_core(bytes: &[u8]) {
        let mut decoded = String::<16>::new();
        let accepted = push_scalars(bytes, decoded.as_mut_view());
        match core::str::from_utf8(bytes) {
            Ok(text) => {
                assert!(accepted, "{bytes:02x?}");
                assert_eq!(decoded.as_str(), text);
            }
            Err(_) => assert!(!accepted, "{bytes:02x?}"),
        }
    }

    #[test]
    fn every_short_sequence_is_judged_as_core_judges_it() {
        for first in 0..=u8::MAX {
            agrees_with_core(&[first]);
            for second in 0..=u8::MAX {
                agrees_with_core(&[first, second]);
                agrees_with_core(&[first, second, 0x80]);
                agrees_with_core(&[first, second, 0xBF, 0x80]);
                agrees_with_core(&[b'a', first, second]);
            }
        }
    }

    #[test]
    fn boundaries_of_every_sequence_length_are_judged_as_core_judges_them() {
        let cases: [&[u8]; 16] = [
            &[0xC0, 0x80],
            &[0xC1, 0xBF],
            &[0xC2, 0x80],
            &[0xE0, 0x9F, 0xBF],
            &[0xE0, 0xA0, 0x80],
            &[0xED, 0x9F, 0xBF],
            &[0xED, 0xA0, 0x80],
            &[0xEF, 0xBF, 0xBF],
            &[0xF0, 0x8F, 0xBF, 0xBF],
            &[0xF0, 0x90, 0x80, 0x80],
            &[0xF4, 0x8F, 0xBF, 0xBF],
            &[0xF4, 0x90, 0x80, 0x80],
            &[0xF5, 0x80, 0x80, 0x80],
            &[0xF8, 0x88, 0x80, 0x80],
            &[0xE2, 0x82],
            &[0xF0, 0x9F, 0x98, 0x80, b'!'],
        ];
        for bytes in cases {
            agrees_with_core(bytes);
        }
    }

    #[test]
    fn a_character_the_text_cannot_take_stops_the_text() {
        let mut decoded = String::<2>::new();
        assert!(!push_scalars(b"abc", decoded.as_mut_view()));
        assert_eq!(decoded.as_str(), "ab");
    }
}
