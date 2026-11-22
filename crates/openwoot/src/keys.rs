const NO_KEY: u8 = 0xFF;
const COLUMN_BITS: u8 = 5;
const COLUMN_MASK: u8 = (1 << COLUMN_BITS) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub row: u8,
    pub column: u8,
}

impl Key {
    #[must_use]
    pub const fn new(row: u8, column: u8) -> Self {
        Self { row, column }
    }

    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        if code == NO_KEY {
            None
        } else {
            Some(Self {
                row: code >> COLUMN_BITS,
                column: code & COLUMN_MASK,
            })
        }
    }

    #[must_use]
    pub const fn code(self) -> u8 {
        self.row << COLUMN_BITS | self.column
    }

    #[must_use]
    pub const fn row(self) -> usize {
        self.row as usize
    }

    #[must_use]
    pub const fn column(self) -> usize {
        self.column as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_code_packs_the_row_above_five_column_bits() {
        assert_eq!(Key::from_code(0xA2), Some(Key::new(5, 2)));
        assert_eq!(Key::from_code(0xFF), None);
        assert_eq!(Key::new(2, 4).code(), 0x44);
    }
}
