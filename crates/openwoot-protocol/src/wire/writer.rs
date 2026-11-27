const CONTINUATION: u8 = 0x80;

pub struct Writer<'a> {
    buffer: &'a mut [u8],
    len: usize,
}

impl<'a> Writer<'a> {
    #[must_use]
    pub const fn new(buffer: &'a mut [u8]) -> Self {
        Self { buffer, len: 0 }
    }

    #[inline]
    pub fn variant(&mut self, index: u32) {
        self.varint(usize::try_from(index).unwrap_or(usize::MAX));
    }

    #[must_use]
    pub fn finish(self) -> Option<usize> {
        (self.len <= self.buffer.len()).then_some(self.len)
    }

    #[inline(never)]
    pub(crate) fn byte(&mut self, byte: u8) -> &mut Self {
        if let Some(slot) = self.buffer.get_mut(self.len) {
            *slot = byte;
        }
        self.len = self.len.saturating_add(1);
        self
    }

    #[inline(never)]
    pub(crate) fn bytes(&mut self, bytes: &[u8]) -> &mut Self {
        for &byte in bytes {
            self.byte(byte);
        }
        self
    }

    #[inline(never)]
    pub(crate) fn text(&mut self, text: &str) -> &mut Self {
        self.varint(text.len());
        self.bytes(text.as_bytes())
    }

    #[inline(never)]
    pub(crate) fn varint(&mut self, mut value: usize) -> &mut Self {
        loop {
            let [low, ..] = value.to_le_bytes();
            value >>= 7;
            if value == 0 {
                return self.byte(low);
            }
            self.byte(low | CONTINUATION);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_match_postcard() {
        for value in [
            0,
            1,
            0x7F,
            0x80,
            0x3FFF,
            0x4000,
            0xFFFF,
            0x1_0000,
            usize::MAX,
        ] {
            let mut expected = [0; 16];
            let expected = postcard::to_slice(&value, &mut expected).unwrap();
            let mut buffer = [0; 16];
            let mut writer = Writer::new(&mut buffer);
            writer.varint(value);
            let len = writer.finish().unwrap();
            assert_eq!(&buffer[..len], expected, "{value:#x}");
        }
    }

    #[test]
    fn a_message_longer_than_the_buffer_fails_as_a_whole() {
        let mut buffer = [0; 2];
        let mut writer = Writer::new(&mut buffer);
        writer.bytes(&[1, 2, 3]);
        writer.byte(4);
        assert_eq!(writer.finish(), None);
        let mut exact = [0; 3];
        let mut writer = Writer::new(&mut exact);
        writer.bytes(&[1, 2, 3]);
        assert_eq!(writer.finish(), Some(3));
    }
}
