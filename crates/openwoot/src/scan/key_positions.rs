use super::{ADDRESSES_PER_INPUT, Map, Position};
use crate::keys::Key;

const CODED_ROWS: usize = 8;
const CODED_COLUMNS: usize = 32;
const UNSENSED: u8 = u8::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyPositions([[u8; CODED_COLUMNS]; CODED_ROWS]);

impl KeyPositions {
    #[must_use]
    pub fn new<const INPUTS: usize, const ADDRESSES: usize>(map: &Map<INPUTS, ADDRESSES>) -> Self {
        const { assert!(INPUTS * ADDRESSES_PER_INPUT <= UNSENSED as usize) };
        let mut positions = [[UNSENSED; CODED_COLUMNS]; CODED_ROWS];
        for (position, key) in map.iter() {
            if let Some(slot) = positions
                .get_mut(key.row())
                .and_then(|row| row.get_mut(key.column()))
            {
                *slot = position.index();
            }
        }
        Self(positions)
    }

    #[must_use]
    #[inline]
    pub fn get(&self, key: Key) -> Option<Position> {
        let index = *self.0.get(key.row())?.get(key.column())?;
        (index != UNSENSED).then(|| Position::from_index(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: Map<2, 3> = Map::from_codes([
        [
            0x44, 0xFF, 0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF,
        ],
        [
            0xFF, 0x01, 0xA2, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF,
        ],
    ]);

    #[test]
    fn every_key_finds_the_sensor_the_map_finds_for_it() {
        let positions = KeyPositions::new(&MAP);
        for row in 0..=u8::MAX {
            for column in 0..=u8::MAX {
                let key = Key::new(row, column);
                assert_eq!(positions.get(key), MAP.position(key), "{key:?}");
            }
        }
        assert_eq!(positions.get(Key::new(2, 4)), Some(Position::new(0, 0)));
    }
}
