use std::time::{Duration, Instant};

const SHORTEST_SAMPLE: Duration = Duration::from_millis(200);
const ROUNDING_HERTZ: u128 = 10;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct ScanRate {
    last: Option<(u32, Instant)>,
    hertz: Option<u32>,
}

impl ScanRate {
    pub(super) fn sample(&mut self, cycles: u32, at: Instant) {
        if let Some((last_cycles, last_at)) = self.last {
            let elapsed = at.saturating_duration_since(last_at);
            if elapsed < SHORTEST_SAMPLE {
                return;
            }
            self.hertz = rate(cycles.wrapping_sub(last_cycles), elapsed);
        }
        self.last = Some((cycles, at));
    }

    pub(super) fn forget(&mut self) {
        *self = Self::default();
    }

    pub(super) const fn hertz(&self) -> Option<u32> {
        self.hertz
    }
}

fn rate(cycles: u32, elapsed: Duration) -> Option<u32> {
    let millis = elapsed.as_millis();
    if cycles == 0 || millis == 0 {
        return None;
    }
    let hertz = u128::from(cycles) * 1000 / millis;
    let rounded = (hertz + ROUNDING_HERTZ / 2) / ROUNDING_HERTZ * ROUNDING_HERTZ;
    u32::try_from(rounded).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rate_is_the_cycles_counted_between_two_samples() {
        let start = Instant::now();
        let mut rate = ScanRate::default();
        rate.sample(1000, start);
        assert_eq!(rate.hertz(), None);
        rate.sample(2253, start + Duration::from_secs(1));
        assert_eq!(rate.hertz(), Some(1250));
        rate.sample(10_253, start + Duration::from_secs(2));
        assert_eq!(rate.hertz(), Some(8000));
    }

    #[test]
    fn a_counter_that_wraps_still_counts_forward() {
        let start = Instant::now();
        let mut rate = ScanRate::default();
        rate.sample(u32::MAX - 99, start);
        rate.sample(900, start + Duration::from_secs(1));
        assert_eq!(rate.hertz(), Some(1000));
    }

    #[test]
    fn a_keyboard_that_does_not_scan_has_no_rate() {
        let start = Instant::now();
        let mut rate = ScanRate::default();
        rate.sample(5, start);
        rate.sample(5, start + Duration::from_secs(1));
        assert_eq!(rate.hertz(), None);
        rate.sample(5, start + Duration::from_millis(1100));
        rate.forget();
        assert_eq!(rate, ScanRate::default());
    }
}
