use openwoot_protocol::diagnostics::StorageUsage;

use super::record::{KEY_LEN, Stored, VERSION_LEN};

#[derive(Debug)]
pub(super) struct Tally {
    usage: StorageUsage,
    header_len: u32,
    word_len: usize,
}

impl Tally {
    pub(super) const fn new(capacity: u32, header_len: u32, word_len: usize) -> Self {
        Self {
            usage: StorageUsage {
                capacity,
                used: 0,
                records: 0,
                items: 0,
            },
            header_len,
            word_len,
        }
    }

    pub(super) fn add(&mut self, stored: Stored<'_>) {
        let data_len = KEY_LEN + VERSION_LEN + stored.body.len();
        let padded = data_len
            .checked_next_multiple_of(self.word_len)
            .unwrap_or(data_len);
        let item_len = self
            .header_len
            .saturating_add(u32::try_from(padded).unwrap_or(u32::MAX));
        self.usage.used = self.usage.used.saturating_add(item_len);
        self.usage.items = self.usage.items.saturating_add(1);
    }

    pub(super) const fn finish(self, records: u32) -> StorageUsage {
        StorageUsage {
            records,
            ..self.usage
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER_LEN: u32 = 8;
    const WORD_LEN: usize = 4;

    fn stored(body: &[u8]) -> Stored<'_> {
        Stored { version: 1, body }
    }

    #[test]
    fn items_take_their_header_and_their_data_padded_to_whole_words() {
        let mut tally = Tally::new(4096, HEADER_LEN, WORD_LEN);
        tally.add(stored(&[7]));
        tally.add(stored(&[7; 2]));
        tally.add(stored(&[]));
        tally.add(stored(&[7; 9]));
        assert_eq!(
            tally.finish(3),
            StorageUsage {
                capacity: 4096,
                used: 12 + 16 + 12 + 20,
                records: 3,
                items: 4,
            }
        );
    }
}
