pub const ADDRESSES_PER_INPUT: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Position {
    input: u8,
    address: u8,
}

impl Position {
    #[must_use]
    pub const fn new(input: u8, address: u8) -> Self {
        Self {
            input,
            address: address % ADDRESSES_PER_INPUT as u8,
        }
    }

    #[must_use]
    pub const fn from_index(index: u8) -> Self {
        Self::new(index / ADDRESSES_PER_INPUT as u8, index)
    }

    #[must_use]
    pub const fn input(self) -> u8 {
        self.input
    }

    #[must_use]
    pub const fn address(self) -> u8 {
        self.address
    }

    #[must_use]
    pub const fn index(self) -> u8 {
        self.address
            .wrapping_add((ADDRESSES_PER_INPUT as u8).wrapping_mul(self.input))
    }

    #[inline(always)]
    fn row(self) -> usize {
        usize::from(self.input)
    }

    #[inline(always)]
    fn column(self) -> usize {
        usize::from(self.address) % ADDRESSES_PER_INPUT
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Grid<T, const INPUTS: usize>([[T; ADDRESSES_PER_INPUT]; INPUTS]);

impl<T, const INPUTS: usize> Grid<T, INPUTS> {
    pub(super) const fn new(cells: [[T; ADDRESSES_PER_INPUT]; INPUTS]) -> Self {
        Self(cells)
    }

    pub(super) fn from_fn(mut cell: impl FnMut(Position) -> T) -> Self {
        Self(core::array::from_fn(|input| {
            core::array::from_fn(|address| cell(Position::new(input as u8, address as u8)))
        }))
    }

    #[inline(always)]
    pub(super) fn get(&self, position: Position) -> Option<&T> {
        self.0
            .get(position.row())
            .and_then(|cells| cells.get(position.column()))
    }

    #[inline(always)]
    pub(super) fn get_mut(&mut self, position: Position) -> Option<&mut T> {
        self.0
            .get_mut(position.row())
            .and_then(|cells| cells.get_mut(position.column()))
    }

    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut().flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scan_position_counts_sixteen_addresses_per_input() {
        assert_eq!(Position::new(5, 14).index(), 94);
        assert_eq!(Position::from_index(94), Position::new(5, 14));
    }

    #[test]
    fn a_grid_holds_one_cell_per_position() {
        let mut grid = Grid::<u8, 2>::from_fn(Position::index);
        assert_eq!(grid.get(Position::new(1, 3)), Some(&19));
        assert_eq!(grid.get(Position::new(2, 0)), None);
        if let Some(cell) = grid.get_mut(Position::new(0, 15)) {
            *cell = 0;
        }
        assert_eq!(
            grid.iter_mut().map(|cell| u32::from(*cell)).sum::<u32>(),
            496 - 15
        );
    }
}
