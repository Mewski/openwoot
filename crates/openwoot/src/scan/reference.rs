use super::Sample;

const DEAD_BAND: u32 = 79;

pub trait Reference: Default {
    fn level(&self, sample: Sample) -> u16;

    fn latch(&mut self, sample: Sample);

    fn latched(&self) -> u16;

    #[inline(always)]
    fn magnitude(&self, sample: Sample) -> u16 {
        let reading = u32::from(sample.reading);
        let level = u32::from(self.level(sample));
        if reading.wrapping_sub(level).wrapping_add(DEAD_BAND) <= 2 * DEAD_BAND {
            return 0;
        }
        reading.abs_diff(level) as u16
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ZeroLevel<const LEVEL: u16>;

impl<const LEVEL: u16> Reference for ZeroLevel<LEVEL> {
    #[inline(always)]
    fn level(&self, _sample: Sample) -> u16 {
        LEVEL
    }

    #[inline(always)]
    fn latch(&mut self, _sample: Sample) {}

    #[inline(always)]
    fn latched(&self) -> u16 {
        0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MeasuredReference<const FALLBACK: u16> {
    latched: u16,
}

impl<const FALLBACK: u16> Reference for MeasuredReference<FALLBACK> {
    #[inline(always)]
    fn level(&self, sample: Sample) -> u16 {
        if sample.reference != 0 {
            sample.reference
        } else {
            FALLBACK
        }
    }

    #[inline(always)]
    fn latch(&mut self, sample: Sample) {
        self.latched = sample.reference;
    }

    #[inline(always)]
    fn latched(&self) -> u16 {
        self.latched
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn sample(reading: u16, reference: u16) -> Sample {
        Sample { reading, reference }
    }

    #[test]
    fn readings_within_the_dead_band_have_no_magnitude() {
        let zero = ZeroLevel::<2048>;
        assert_eq!(zero.magnitude(sample(2048 + 79, 0)), 0);
        assert_eq!(zero.magnitude(sample(2048 - 79, 0)), 0);
        assert_eq!(zero.magnitude(sample(2048 + 80, 0)), 80);
        assert_eq!(zero.magnitude(sample(2048 - 80, 0)), 80);
        assert_eq!(zero.magnitude(sample(0, 0)), 2048);
    }

    #[test]
    fn a_measured_reference_replaces_the_zero_level_unless_it_reads_zero() {
        let mut reference = MeasuredReference::<2482>::default();
        assert_eq!(reference.magnitude(sample(2600, 2400)), 200);
        assert_eq!(reference.magnitude(sample(2600, 0)), 118);
        reference.latch(sample(2600, 2400));
        assert_eq!(reference.latched(), 2400);
    }
}
