mod min_max;
mod standard_deviation;
mod welford;

pub use min_max::MinMax;
pub use standard_deviation::StandardDeviation;
use welford::Welford;

const SAMPLES_DONE: u16 = 1000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoiseState {
    #[default]
    Idle,
    Sampling,
    Done,
    Unstable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calibration {
    pub rest_level: u16,
    pub noise_band: u16,
}

pub trait NoiseStatistics: Clone + Default {
    const IDLE: Self;

    fn restart(&mut self);

    fn add(&mut self, magnitude: u16) -> NoiseState;

    fn state(&self) -> NoiseState;

    fn calibration(&self, noise_band: u16) -> Calibration;
}
