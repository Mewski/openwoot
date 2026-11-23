use super::{ADDRESSES_PER_INPUT, Grid, Position, Sensor};
use crate::analog::NoiseStatistics;

#[derive(Debug)]
pub struct Sensors<Noise, const INPUTS: usize>(Grid<Sensor<Noise>, INPUTS>);

impl<Noise: NoiseStatistics, const INPUTS: usize> Default for Sensors<Noise, INPUTS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Noise: NoiseStatistics, const INPUTS: usize> Sensors<Noise, INPUTS> {
    #[must_use]
    pub const fn new() -> Self {
        Self(Grid::new(
            [const { [const { Sensor::new() }; ADDRESSES_PER_INPUT] }; INPUTS],
        ))
    }

    #[inline(always)]
    pub(super) fn get(&self, position: Position) -> Option<&Sensor<Noise>> {
        self.0.get(position)
    }

    #[inline(always)]
    pub(super) fn get_mut(&mut self, position: Position) -> Option<&mut Sensor<Noise>> {
        self.0.get_mut(position)
    }

    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = &mut Sensor<Noise>> {
        self.0.iter_mut()
    }
}
