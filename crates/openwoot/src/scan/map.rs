use super::{ADDRESSES_PER_INPUT, Grid, Position};
use crate::keys::Key;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Map<const INPUTS: usize, const ADDRESSES: usize> {
    codes: Grid<u8, INPUTS>,
}

impl<const INPUTS: usize, const ADDRESSES: usize> Map<INPUTS, ADDRESSES> {
    #[must_use]
    #[expect(
        clippy::indexing_slicing,
        reason = "evaluated at compile time, where an index outside the table fails the build"
    )]
    pub const fn from_codes(codes: [[u8; ADDRESSES_PER_INPUT]; INPUTS]) -> Self {
        const { assert!(ADDRESSES <= ADDRESSES_PER_INPUT && INPUTS <= u8::MAX as usize) };
        let mut sensed = [false; 256];
        let mut input = 0;
        while input < INPUTS {
            let mut address = 0;
            while address < ADDRESSES_PER_INPUT {
                let code = codes[input][address];
                if Key::from_code(code).is_some() {
                    assert!(
                        address < ADDRESSES,
                        "a key behind an address the scan never selects"
                    );
                    assert!(!sensed[code as usize], "a key behind two sensors");
                    sensed[code as usize] = true;
                }
                address += 1;
            }
            input += 1;
        }
        Self {
            codes: Grid::new(codes),
        }
    }

    #[must_use]
    #[inline]
    pub fn key(&self, position: Position) -> Option<Key> {
        self.codes
            .get(position)
            .and_then(|&code| Key::from_code(code))
    }

    #[must_use]
    pub fn position(&self, key: Key) -> Option<Position> {
        self.iter()
            .find_map(|(position, found)| (found == key).then_some(position))
    }

    pub fn iter(&self) -> impl Iterator<Item = (Position, Key)> + '_ {
        (0..ADDRESSES as u8).flat_map(move |address| {
            (0..INPUTS as u8).filter_map(move |input| {
                let position = Position::new(input, address);
                self.key(position).map(|key| (position, key))
            })
        })
    }

    #[must_use]
    pub fn input_has_keys(&self, input: u8) -> bool {
        (0..ADDRESSES as u8).any(|address| self.key(Position::new(input, address)).is_some())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

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
    fn the_map_finds_keys_by_position_and_positions_by_key() {
        assert_eq!(MAP.key(Position::new(1, 2)), Some(Key::new(5, 2)));
        assert_eq!(MAP.key(Position::new(1, 0)), None);
        assert_eq!(MAP.position(Key::new(0, 1)), Some(Position::new(1, 1)));
        assert_eq!(MAP.len(), 4);
        assert!(MAP.input_has_keys(1));
    }

    #[test]
    fn the_map_lists_keys_address_by_address() {
        let positions: Vec<_> = MAP.iter().map(|(position, _)| position.index()).collect();
        assert_eq!(positions, [0, 17, 2, 18]);
    }

    #[test]
    #[should_panic(expected = "a key behind two sensors")]
    fn a_key_behind_two_sensors_is_rejected() {
        let mut codes = [[0xFF; ADDRESSES_PER_INPUT]; 2];
        codes[0][1] = 0x44;
        codes[1][0] = 0x44;
        let _ = Map::<2, 3>::from_codes(codes);
    }
}
