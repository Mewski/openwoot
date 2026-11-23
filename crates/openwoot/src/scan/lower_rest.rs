const NOISE_BANDS_BELOW: u32 = 4;
const MARGIN_BELOW: u32 = 10;
const SETTLE_MS: u16 = 64;
const SETTLE_SAMPLES: u8 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LowerRest {
    low: u16,
    high: u16,
    start_ms: u16,
    samples: u8,
}

impl LowerRest {
    pub(super) const fn new() -> Self {
        Self {
            low: 0,
            high: 0,
            start_ms: 0,
            samples: 0,
        }
    }

    pub(super) const fn clear(&mut self) {
        self.samples = 0;
    }

    #[inline(always)]
    pub(super) fn observe(
        &mut self,
        value: u16,
        rest_level: u16,
        noise_band: u16,
        now_ms: u16,
    ) -> Option<u16> {
        let depth = u32::from(noise_band) * NOISE_BANDS_BELOW + MARGIN_BELOW;
        if u32::from(value) + depth > u32::from(rest_level) {
            self.samples = 0;
            return None;
        }
        self.settle(value, noise_band, now_ms)
    }

    #[inline(never)]
    fn settle(&mut self, value: u16, noise_band: u16, now_ms: u16) -> Option<u16> {
        let low = self.low.min(value);
        let high = self.high.max(value);
        if self.samples == 0 || high.abs_diff(low) > noise_band {
            *self = Self {
                low: value,
                high: value,
                start_ms: now_ms,
                samples: 1,
            };
            return None;
        }
        self.low = low;
        self.high = high;
        self.samples = self.samples.saturating_add(1);
        if self.samples < SETTLE_SAMPLES || now_ms.wrapping_sub(self.start_ms) < SETTLE_MS {
            return None;
        }
        self.samples = 0;
        Some(high)
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    const REST: u16 = 1102;
    const BAND: u16 = 2;

    fn feed(
        lower_rest: &mut LowerRest,
        values: impl IntoIterator<Item = (u16, u16)>,
    ) -> Option<u16> {
        values
            .into_iter()
            .find_map(|(now_ms, value)| lower_rest.observe(value, REST, BAND, now_ms))
    }

    fn every_millisecond(from_ms: u16, values: impl IntoIterator<Item = u16>) -> Vec<(u16, u16)> {
        values
            .into_iter()
            .enumerate()
            .map(|(index, value)| (from_ms.wrapping_add(index as u16), value))
            .collect()
    }

    #[test]
    fn a_level_well_below_rest_held_for_the_settle_time_becomes_the_rest_level() {
        let mut lower_rest = LowerRest::new();
        let readings = every_millisecond(0, (0..64).map(|index| 300 + index % 2));
        assert_eq!(feed(&mut lower_rest, readings), None);
        assert_eq!(lower_rest.observe(300, REST, BAND, 64), Some(301));
    }

    #[test]
    fn a_single_sample_below_rest_moves_nothing() {
        let mut lower_rest = LowerRest::new();
        let mut readings = every_millisecond(0, [REST; 40]);
        readings.extend(every_millisecond(40, [0]));
        readings.extend(every_millisecond(41, [REST; 200]));
        assert_eq!(feed(&mut lower_rest, readings), None);
    }

    #[test]
    fn a_bounce_below_rest_moves_nothing() {
        let mut lower_rest = LowerRest::new();
        let bounce = (0..500).map(|index| if index % 7 < 3 { 300 } else { 400 });
        assert_eq!(feed(&mut lower_rest, every_millisecond(0, bounce)), None);
    }

    #[test]
    fn a_level_that_stays_below_rest_for_less_than_the_settle_time_moves_nothing() {
        let mut lower_rest = LowerRest::new();
        let mut readings = every_millisecond(0, [300; 64]);
        readings.extend(every_millisecond(64, [REST; 200]));
        assert_eq!(feed(&mut lower_rest, readings), None);
    }

    #[test]
    fn a_level_within_four_noise_bands_and_the_margin_of_rest_moves_nothing() {
        let mut lower_rest = LowerRest::new();
        let shallow = REST - 4 * BAND - 10 + 1;
        assert_eq!(
            feed(&mut lower_rest, every_millisecond(0, [shallow; 500])),
            None
        );
        let deep = shallow - 1;
        assert_eq!(
            feed(&mut lower_rest, every_millisecond(500, [deep; 65])),
            Some(deep)
        );
    }

    #[test]
    fn a_settle_time_with_too_few_samples_is_not_enough() {
        let mut lower_rest = LowerRest::new();
        let sparse = (0..31_u16).map(|index| (index * 10, 300));
        assert_eq!(feed(&mut lower_rest, sparse), None);
        assert_eq!(lower_rest.observe(300, REST, BAND, 310), Some(300));
    }

    #[test]
    fn the_settle_time_is_measured_across_the_millisecond_wrap() {
        let mut lower_rest = LowerRest::new();
        let readings = every_millisecond(u16::MAX - 10, [300; 64]);
        assert_eq!(feed(&mut lower_rest, readings), None);
        assert_eq!(lower_rest.observe(300, REST, BAND, 53), Some(300));
    }

    #[test]
    fn an_unmeasured_noise_band_never_lowers_the_rest_level() {
        let mut lower_rest = LowerRest::new();
        let readings =
            (0..1000_u16).map(|now_ms| lower_rest.observe(0, u16::MAX, u16::MAX, now_ms));
        assert!(readings.into_iter().all(|rest_level| rest_level.is_none()));
    }

    #[test]
    fn a_gap_in_the_readings_restarts_the_settle_time() {
        let mut lower_rest = LowerRest::new();
        assert_eq!(feed(&mut lower_rest, every_millisecond(0, [300; 60])), None);
        lower_rest.clear();
        assert_eq!(
            feed(&mut lower_rest, every_millisecond(60, [300; 10])),
            None
        );
    }
}
