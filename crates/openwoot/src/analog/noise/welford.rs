const CHECK_FIRST: i32 = 500;
const CHECK_STEP: i32 = 100;
const CHECKS_MAX: u8 = 9;
const DRIFT_LIMIT: f32 = 0.5;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Welford {
    count: u16,
    mean: f32,
    sum_of_squares: f32,
    last_deviation: f32,
    drift: f32,
    checks: u8,
}

impl Welford {
    pub(super) const WARMUP_SAMPLES: u16 = 400;

    pub(super) const fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            sum_of_squares: 0.0,
            last_deviation: 0.0,
            drift: 0.0,
            checks: 0,
        }
    }

    pub(super) const fn count(&self) -> u16 {
        self.count
    }

    pub(super) const fn mean(&self) -> f32 {
        self.mean
    }

    pub(super) fn add(&mut self, value: f32) {
        self.count = self.count.wrapping_add(1);
        let delta = value - self.mean;
        self.mean += delta / f32::from(self.count);
        self.sum_of_squares += delta * (value - self.mean);
    }

    pub(super) fn check_due(&self) -> bool {
        self.count > Self::WARMUP_SAMPLES
            && i32::from(self.count) > CHECK_STEP * i32::from(self.checks) + CHECK_FIRST
            && self.checks <= CHECKS_MAX
    }

    pub(super) fn check_drifts(&mut self) -> bool {
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

    pub(super) fn deviation(&self) -> f32 {
        libm::sqrtf(self.sum_of_squares / f32::from(self.count.wrapping_sub(1)))
    }
}
