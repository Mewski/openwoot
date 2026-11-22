mod min_max;
mod standard_deviation;

pub use min_max::MinMax;
pub use standard_deviation::StdDev;

const SAMPLES_DONE: u16 = 1000;
const WARMUP_SAMPLES: u16 = 400;
const CHECK_FIRST: i32 = 500;
const CHECK_STEP: i32 = 100;
const CHECKS_MAX: u8 = 9;
const DRIFT_LIMIT: f32 = 0.5;

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
    fn restart(&mut self);

    fn add(&mut self, magnitude: u16) -> NoiseState;

    fn state(&self) -> NoiseState;

    fn calibration(&self, noise_band: u16) -> Calibration;
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Welford {
    count: u16,
    mean: f32,
    sum_of_squares: f32,
    last_deviation: f32,
    drift: f32,
    checks: u8,
}

impl Welford {
    const fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            sum_of_squares: 0.0,
            last_deviation: 0.0,
            drift: 0.0,
            checks: 0,
        }
    }

    fn add(&mut self, value: f32) {
        self.count = self.count.wrapping_add(1);
        let delta = value - self.mean;
        self.mean += delta / f32::from(self.count);
        self.sum_of_squares += delta * (value - self.mean);
    }

    fn check_due(&self) -> bool {
        self.count > WARMUP_SAMPLES
            && i32::from(self.count) > CHECK_STEP * i32::from(self.checks) + CHECK_FIRST
            && self.checks <= CHECKS_MAX
    }

    fn check_drifts(&mut self) -> bool {
        let deviation = self.deviation();
        if self.checks != 0 {
            self.drift += deviation - self.last_deviation;
            if self.drift.abs() > DRIFT_LIMIT {
                return true;
            }
        }
        self.checks += 1;
        self.last_deviation = deviation;
        false
    }

    fn deviation(&self) -> f32 {
        libm::sqrtf(self.sum_of_squares / f32::from(self.count.wrapping_sub(1)))
    }
}
