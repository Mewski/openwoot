use core::cmp::Ordering;

use openwoot_protocol::gamepad::Settings;

use super::normalize_travel;

const BOTTOM_OUT_LIMIT: u16 = 4013;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AxisPriority {
    #[default]
    None,
    Positive,
    Negative,
    PositiveTookOver,
    NegativeTookOver,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Directions {
    pub(super) up: u16,
    pub(super) down: u16,
    pub(super) left: u16,
    pub(super) right: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Positive,
    Negative,
}

pub fn combine_axis(
    positive: u16,
    negative: u16,
    priority: &mut AxisPriority,
    settings: Option<&Settings>,
) -> f32 {
    let positive_level = level(positive, settings);
    let negative_level = level(negative, settings);
    if !settings.is_some_and(|settings| settings.single_direction_axes) {
        return positive_level - negative_level;
    }
    match priority.resolve(positive, negative) {
        Some(Side::Positive) => positive_level,
        Some(Side::Negative) => -negative_level,
        None => 0.0,
    }
}

impl AxisPriority {
    fn resolve(&mut self, positive: u16, negative: u16) -> Option<Side> {
        if positive == 0 && negative == 0 {
            *self = Self::None;
            return None;
        }
        match leader(positive, negative) {
            Some(Side::Positive) => {
                *self = Self::Positive;
                Some(Side::Positive)
            }
            Some(Side::Negative) => {
                *self = Self::Negative;
                Some(Side::Negative)
            }
            None => self.take_over(),
        }
    }

    fn take_over(&mut self) -> Option<Side> {
        match self {
            Self::None => None,
            Self::Positive => {
                *self = Self::NegativeTookOver;
                Some(Side::Negative)
            }
            Self::Negative => {
                *self = Self::PositiveTookOver;
                Some(Side::Positive)
            }
            Self::PositiveTookOver => Some(Side::Positive),
            Self::NegativeTookOver => Some(Side::Negative),
        }
    }
}

impl Directions {
    pub(super) const fn new() -> Self {
        Self {
            up: 0,
            down: 0,
            left: 0,
            right: 0,
        }
    }

    pub(super) fn combine(
        self,
        [x, y]: &mut [AxisPriority; 2],
        settings: Option<&Settings>,
    ) -> (f32, f32) {
        (
            combine_axis(self.right, self.left, x, settings),
            combine_axis(self.down, self.up, y, settings),
        )
    }
}

fn level(travel: u16, settings: Option<&Settings>) -> f32 {
    if travel == 0 {
        0.0
    } else {
        normalize_travel(u32::from(travel), settings)
    }
}

fn leader(positive: u16, negative: u16) -> Option<Side> {
    match (bottomed_out(positive), bottomed_out(negative)) {
        (true, true) => None,
        (true, false) => Some(Side::Positive),
        (false, true) => Some(Side::Negative),
        (false, false) => match positive.cmp(&negative) {
            Ordering::Greater => Some(Side::Positive),
            Ordering::Less => Some(Side::Negative),
            Ordering::Equal => None,
        },
    }
}

const fn bottomed_out(travel: u16) -> bool {
    travel > BOTTOM_OUT_LIMIT
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::gamepad::{CurvePoint, Normalization};

    use super::*;

    const SINGLE_DIRECTION: Settings = Settings {
        curve: [
            CurvePoint {
                travel: 0,
                output: 0,
            },
            CurvePoint {
                travel: 85,
                output: 85,
            },
            CurvePoint {
                travel: 170,
                output: 170,
            },
            CurvePoint {
                travel: 255,
                output: 255,
            },
        ],
        normalization: Normalization::Circular,
        single_direction_axes: true,
        left_stick_up_bias: None,
    };

    fn combine(positive: u16, negative: u16, before: AxisPriority) -> (f32, AxisPriority) {
        let mut priority = before;
        let value = combine_axis(positive, negative, &mut priority, Some(&SINGLE_DIRECTION));
        (value, priority)
    }

    #[test]
    fn without_single_direction_axes_both_directions_count() {
        let mut priority = AxisPriority::Negative;
        let value = combine_axis(4095, 2048, &mut priority, None);
        assert_eq!(value, 1.0 - 2048.0 / 4095.0);
        assert_eq!(priority, AxisPriority::Negative);
    }

    #[test]
    fn the_deeper_direction_wins() {
        assert_eq!(
            combine(2048, 100, AxisPriority::None),
            (2048.0 / 4095.0, AxisPriority::Positive)
        );
        assert_eq!(
            combine(100, 2048, AxisPriority::Positive),
            (-2048.0 / 4095.0, AxisPriority::Negative)
        );
        assert_eq!(
            combine(4014, 4013, AxisPriority::Negative),
            (4014.0 / 4095.0, AxisPriority::Positive)
        );
    }

    #[test]
    fn when_both_bottom_out_the_other_direction_takes_over() {
        assert_eq!(
            combine(4095, 4014, AxisPriority::Positive),
            (-4014.0 / 4095.0, AxisPriority::NegativeTookOver)
        );
        assert_eq!(
            combine(4014, 4095, AxisPriority::Negative),
            (4014.0 / 4095.0, AxisPriority::PositiveTookOver)
        );
        assert_eq!(
            combine(4095, 4095, AxisPriority::PositiveTookOver),
            (1.0, AxisPriority::PositiveTookOver)
        );
        assert_eq!(
            combine(2000, 2000, AxisPriority::NegativeTookOver),
            (-2000.0 / 4095.0, AxisPriority::NegativeTookOver)
        );
        assert_eq!(
            combine(4095, 4095, AxisPriority::None),
            (0.0, AxisPriority::None)
        );
    }

    #[test]
    fn releasing_both_directions_forgets_the_winner() {
        assert_eq!(
            combine(0, 0, AxisPriority::NegativeTookOver),
            (0.0, AxisPriority::None)
        );
    }
}
