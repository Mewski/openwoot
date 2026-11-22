use super::{Calibration, NoiseState, NoiseStatistics, SAMPLES_DONE, Welford};

const AVERAGE_KEEP: f32 = 0.95;
const AVERAGE_NEW: f32 = 1.0 - AVERAGE_KEEP;
const MOVEMENT_SAMPLES: u16 = 25;
const MOVEMENT_LIMIT: f32 = 4.0;
const BAND_PER_DEVIATION: f64 = 2.8;
const NEW_WEIGHT: f64 = 0.9;
const REST_OFFSET: u32 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StdDev {
    welford: Welford,
    average: f32,
    state: NoiseState,
}

impl NoiseStatistics for StdDev {
    fn restart(&mut self) {
        *self = Self {
            state: NoiseState::Sampling,
            ..Self::default()
        };
    }

    fn add(&mut self, magnitude: u16) -> NoiseState {
        if self.state != NoiseState::Sampling {
            return self.state;
        }
        let value = f32::from(magnitude);
        self.welford.add(value);
        let count = self.welford.count;
        if count == 1 {
            self.average = value;
            return NoiseState::Sampling;
        }
        self.average = value * AVERAGE_NEW + self.average * AVERAGE_KEEP;
        if count > MOVEMENT_SAMPLES {
            if (self.average - self.welford.mean).abs() > MOVEMENT_LIMIT {
                self.state = NoiseState::Unstable;
                return self.state;
            }
            if self.welford.check_due() && self.welford.check_drifts() {
                self.state = NoiseState::Unstable;
                return self.state;
            }
        }
        if count >= SAMPLES_DONE {
            self.state = NoiseState::Done;
        }
        self.state
    }

    fn state(&self) -> NoiseState {
        self.state
    }

    fn calibration(&self, noise_band: u16) -> Calibration {
        let measured = if self.welford.count > 1 {
            f64::from((f64::from(self.welford.deviation()) * BAND_PER_DEVIATION) as f32)
                * NEW_WEIGHT
        } else {
            0.0
        };
        let noise = (f64::from(f32::from(noise_band)) * (1.0 - NEW_WEIGHT) + measured) as f32;
        Calibration {
            noise_band: libm::ceilf(noise) as u32 as u16,
            rest_level: (libm::ceilf(self.welford.mean) as u32).wrapping_add(REST_OFFSET) as u16,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(samples: impl IntoIterator<Item = u16>) -> (StdDev, NoiseState) {
        let mut statistics = StdDev::default();
        statistics.restart();
        let mut state = NoiseState::Sampling;
        for sample in samples {
            state = statistics.add(sample);
            if state != NoiseState::Sampling {
                break;
            }
        }
        (statistics, state)
    }

    #[test]
    fn a_quiet_window_yields_mean_and_deviation() {
        let (statistics, state) = window((0..1000).map(|index| 10 + (index % 2) as u16));
        assert_eq!(state, NoiseState::Done);
        let calibration = statistics.calibration(4);
        assert_eq!(calibration.rest_level, 11 + 2);
        assert_eq!(calibration.noise_band, 2);
    }

    #[test]
    fn a_key_that_starts_to_move_ends_the_window() {
        let (statistics, state) = window((0..1000).map(|index| if index < 100 { 10 } else { 40 }));
        assert_eq!(state, NoiseState::Unstable);
        assert!(statistics.welford.count < 120);
    }

    #[test]
    fn the_moving_average_weight_is_the_single_precision_complement() {
        assert_eq!(AVERAGE_NEW.to_bits(), (1.0_f32 - 0.95_f32).to_bits());
        assert_ne!(AVERAGE_NEW.to_bits(), 0.05_f32.to_bits());
    }
}
