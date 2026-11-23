use super::{Calibration, NoiseState, NoiseStatistics, SAMPLES_DONE, Welford};
use crate::analog::Switch;

const MIN_RESET: u16 = 0x0FFF;
const BAND_PER_RANGE: f64 = 0.52;
const NEW_WEIGHT: f64 = 0.9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MinMax {
    max: u16,
    min: u16,
    welford: Welford,
    state: NoiseState,
}

impl Default for MinMax {
    fn default() -> Self {
        Self::IDLE
    }
}

impl NoiseStatistics for MinMax {
    const IDLE: Self = Self {
        max: 0,
        min: MIN_RESET,
        welford: Welford::new(),
        state: NoiseState::Idle,
    };

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
        if magnitude > self.max {
            self.max = magnitude;
        }
        if magnitude < self.min {
            self.min = magnitude;
        }
        self.welford.add(f32::from(magnitude));
        let count = self.welford.count();
        if count <= Welford::WARMUP_SAMPLES {
            return NoiseState::Sampling;
        }
        let complete = count >= SAMPLES_DONE;
        if !complete && !self.welford.check_due() {
            return self.state;
        }
        if self.welford.check_drifts() {
            self.state = NoiseState::Unstable;
            return self.state;
        }
        if complete {
            self.state = NoiseState::Done;
        }
        self.state
    }

    fn state(&self) -> NoiseState {
        self.state
    }

    fn calibration(&self, noise_band: u16) -> Calibration {
        let min = i32::from(self.min);
        let half_range = (f64::from((i32::from(self.max) - min) as f32) * BAND_PER_RANGE) as f32;
        let noise = (f64::from(half_range) * NEW_WEIGHT
            + f64::from(f32::from(noise_band)) * (1.0 - NEW_WEIGHT)) as f32;
        let noise_band = libm::ceilf(noise) as u32 as u16;
        let rest_level = (libm::ceilf(min as f32 + noise) as u32)
            .wrapping_add(u32::from(Switch::REST_MARGIN)) as u16;
        Calibration {
            rest_level,
            noise_band,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(samples: impl IntoIterator<Item = u16>) -> (MinMax, NoiseState) {
        let mut statistics = MinMax::default();
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
    fn a_quiet_window_completes_after_a_thousand_samples() {
        let (statistics, state) = window((0..2000).map(|index| 3 + (index % 3) as u16));
        assert_eq!(state, NoiseState::Done);
        assert_eq!(statistics.welford.count(), 1000);
        assert_eq!(statistics.max, 5);
        assert_eq!(statistics.min, 3);
    }

    #[test]
    fn the_band_follows_the_spread_and_the_rest_level_the_minimum() {
        let (statistics, _) = window((0..1000).map(|index| 3 + (index % 3) as u16));
        let calibration = statistics.calibration(10);
        assert_eq!(
            calibration,
            Calibration {
                noise_band: 2,
                rest_level: 3 + 2 + 5,
            }
        );
    }

    #[test]
    fn a_window_whose_spread_keeps_growing_is_unstable() {
        let (_, state) = window((0..1000_u16).map(|index| {
            let amplitude = index / 50;
            if index % 2 == 0 {
                100 + amplitude
            } else {
                100 - amplitude
            }
        }));
        assert_eq!(state, NoiseState::Unstable);
    }

    #[test]
    fn a_level_change_in_the_last_tenth_of_the_window_is_unstable() {
        for change_at in [905, 950, 999] {
            let (statistics, state) = window((0..1000_u16).map(|index| {
                if index < change_at {
                    1100 + index % 3
                } else {
                    300 + index % 3
                }
            }));
            assert_eq!(state, NoiseState::Unstable, "change at {change_at}");
            assert_eq!(statistics.welford.count(), 1000);
        }
    }

    #[test]
    fn the_first_sample_counts_for_the_minimum_too() {
        let (statistics, state) = window((0..1000_u16).map(|index| 4 + u16::from(index != 0)));
        assert_eq!(state, NoiseState::Done);
        assert_eq!((statistics.min, statistics.max), (4, 5));
    }

    #[test]
    fn statistics_take_samples_only_while_sampling() {
        let mut statistics = MinMax::default();
        assert_eq!(statistics.add(7), NoiseState::Idle);
        assert_eq!(statistics.max, 0);
    }
}
