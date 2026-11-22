const POINTS: usize = 4096;
const COUNT: usize = 40;

const STEPS: &[u8; COUNT * POINTS] = include_bytes!("../../data/travel_curves.txt");
static CURVES: [[u16; POINTS]; COUNT] = decode(STEPS);

#[derive(Clone, Copy, Debug)]
pub struct Curve(&'static [u16; POINTS]);

impl Curve {
    pub(super) const END_MAX: u32 = 0xF_FF00;
    pub(super) const VALUE_MAX: u32 = 0x3FFF;

    #[must_use]
    pub(super) fn number(number: u8) -> Self {
        match CURVES.get(usize::from(number)) {
            Some(points) => Self(points),
            None => Self(&CURVES[0]),
        }
    }

    #[must_use]
    pub fn point(self, index: u32) -> u16 {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.0.get(index))
            .copied()
            .unwrap_or(0)
    }

    #[must_use]
    pub fn interpolate(self, index_q8: u32) -> u16 {
        let index = (index_q8 >> 8) & 0xFFFF;
        let fraction = index_q8 & 0xFF;
        let low = u32::from(self.point(index));
        if fraction == 0 {
            return low as u16;
        }
        let high = u32::from(self.point(index + 1));
        (low + (high
            .wrapping_sub(low)
            .wrapping_mul(fraction)
            .wrapping_add(128)
            >> 8)) as u16
    }

    #[must_use]
    pub(super) fn end_of_travel(self, target: u32) -> u32 {
        let last = POINTS as u32 - 1;
        let mut low = 0_u32;
        let mut high = last;
        loop {
            let middle = low + ((high - low) >> 1);
            let point = u32::from(self.point(middle));
            match point.cmp(&target) {
                core::cmp::Ordering::Less => low = middle + 1,
                core::cmp::Ordering::Greater => match middle.checked_sub(1) {
                    Some(below) => high = below,
                    None => return 0,
                },
                core::cmp::Ordering::Equal => return self.plateau_end(middle, target),
            }
            if low > high {
                break;
            }
        }
        if low == 0 {
            0
        } else if low > last {
            Self::END_MAX
        } else {
            let below = u32::from(self.point(high));
            let step = u32::from(self.point(low)).wrapping_sub(below) & 0xFFFF;
            ((step >> 1) + ((target.wrapping_sub(below)) << 8))
                .checked_div(step)
                .map_or(Self::END_MAX, |position| {
                    (position + (high << 8)).min(Self::END_MAX)
                })
        }
    }

    fn plateau_end(self, hit: u32, target: u32) -> u32 {
        let mut first = hit;
        while first != 0 && u32::from(self.point(first - 1)) == target {
            first -= 1;
        }
        let mut last = hit;
        while last + 1 < POINTS as u32 && u32::from(self.point(last + 1)) == target {
            last += 1;
        }
        ((first + (last - first) * target / Self::VALUE_MAX) << 8).min(Self::END_MAX)
    }
}

impl PartialEq for Curve {
    fn eq(&self, other: &Self) -> bool {
        core::ptr::eq(self.0, other.0)
    }
}

impl Eq for Curve {}

#[expect(
    clippy::indexing_slicing,
    clippy::large_stack_arrays,
    reason = "evaluated at compile time, where an index outside the table fails the build"
)]
const fn decode(steps: &[u8; COUNT * POINTS]) -> [[u16; POINTS]; COUNT] {
    let mut curves = [[0; POINTS]; COUNT];
    let mut curve = 0;
    while curve < COUNT {
        let line = curve * POINTS;
        assert!(
            steps[line + POINTS - 1] == b'\n',
            "one line of steps per curve"
        );
        let mut point = 1;
        while point < POINTS {
            let step = steps[line + point - 1];
            assert!(
                step >= b'A' && step <= b'Z',
                "a step is a letter from `A` (0) upwards"
            );
            curves[curve][point] = curves[curve][point - 1] + (step - b'A') as u16;
            point += 1;
        }
        curve += 1;
    }
    curves
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curve_rises_from_rest_to_full_travel() {
        for number in 0..COUNT as u8 {
            let curve = Curve::number(number);
            assert_eq!(curve.point(0), 0);
            assert_eq!(u32::from(curve.point(POINTS as u32 - 1)), Curve::VALUE_MAX);
            for index in 1..POINTS as u32 {
                assert!(curve.point(index - 1) <= curve.point(index));
            }
        }
    }

    #[test]
    fn interpolation_rounds_between_neighbouring_points() {
        let curve = Curve::number(0);
        assert_eq!(curve.interpolate(0x100), 17);
        assert_eq!(curve.interpolate(0x180), 26);
        assert_eq!(curve.interpolate(0x140), 21);
        assert_eq!(curve.interpolate(Curve::END_MAX), 0x3FFF);
    }

    #[test]
    fn the_end_of_travel_of_the_full_value_is_the_start_of_the_last_plateau() {
        let curve = Curve::number(0);
        let end = curve.end_of_travel(Curve::VALUE_MAX);
        let first = (0..POINTS as u32)
            .find(|&index| u32::from(curve.point(index)) == Curve::VALUE_MAX)
            .unwrap();
        assert_eq!(end, ((first + (4095 - first)) << 8).min(Curve::END_MAX));
        assert_eq!(curve.end_of_travel(0), 0);
    }
}
