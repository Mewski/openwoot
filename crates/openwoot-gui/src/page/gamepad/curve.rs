use openwoot_protocol::gamepad::{CURVE_POINTS, CurvePoint, Settings};

pub(super) type Curve = [CurvePoint; CURVE_POINTS];

pub(super) const POINT_SCALE: f32 = 255.0;
pub(super) const FULL_TRAVEL: f32 = 4095.0;
pub(super) const DEFAULT_UP_BIAS: u8 = 50;
const MAX_PERCENT: u8 = 100;

pub(super) struct Preset {
    pub(super) name: &'static str,
    pub(super) curve: Curve,
}

pub(super) const PRESETS: [Preset; 7] = [
    Preset {
        name: "Linear",
        curve: [
            point(0, 20),
            point(60, 60),
            point(194, 195),
            point(253, 255),
        ],
    },
    Preset {
        name: "Aggro",
        curve: [
            point(0, 33),
            point(63, 114),
            point(139, 231),
            point(253, 255),
        ],
    },
    Preset {
        name: "Slow",
        curve: [
            point(0, 23),
            point(128, 63),
            point(226, 145),
            point(253, 255),
        ],
    },
    Preset {
        name: "Smooth",
        curve: [
            point(0, 26),
            point(78, 84),
            point(199, 174),
            point(253, 255),
        ],
    },
    Preset {
        name: "Steep",
        curve: [
            point(0, 26),
            point(25, 132),
            point(192, 135),
            point(253, 255),
        ],
    },
    Preset {
        name: "Instant",
        curve: [
            point(0, 255),
            point(2, 255),
            point(192, 255),
            point(255, 255),
        ],
    },
    Preset {
        name: "Turbo",
        curve: [point(0, 255), point(1, 255), point(2, 255), point(3, 255)],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(super) enum Problem {
    #[error("the curve points must rise from the dead zone to the saturation point")]
    CurveNotRising,
    #[error("the dead zone must lie below the saturation point")]
    CurveWithoutSpan,
    #[error("the joystick angle of {0}% lies above 100%")]
    BiasAboveHundred(u8),
}

pub(super) fn check(settings: &Settings) -> Result<(), Problem> {
    let rising = settings
        .curve
        .windows(2)
        .all(|pair| matches!(pair, [low, high] if low.travel <= high.travel));
    if !rising {
        return Err(Problem::CurveNotRising);
    }
    let spans = matches!(
        (settings.curve.first(), settings.curve.last()),
        (Some(dead_zone), Some(saturation)) if dead_zone.travel < saturation.travel
    );
    if !spans {
        return Err(Problem::CurveWithoutSpan);
    }
    match settings.left_stick_up_bias {
        Some(percent) if percent > MAX_PERCENT => Err(Problem::BiasAboveHundred(percent)),
        _ => Ok(()),
    }
}

pub(super) fn preset_of(curve: Curve) -> Option<usize> {
    PRESETS.iter().position(|preset| preset.curve == curve)
}

pub(super) fn move_point(curve: &mut Curve, index: usize, travel: u8, output: u8) {
    let last = CURVE_POINTS - 1;
    let travel_of = |position: usize| curve.get(position).map_or(0, |point| point.travel);
    let (lower, upper) = match index {
        0 => (0, travel_of(1).min(travel_of(last).saturating_sub(1))),
        position if position == last => (
            travel_of(last - 1).max(travel_of(0).saturating_add(1)),
            u8::MAX,
        ),
        position => (travel_of(position - 1), travel_of(position + 1)),
    };
    if let Some(point) = curve.get_mut(index) {
        point.travel = travel.max(lower).min(upper);
        point.output = output;
    }
}

pub(super) fn travel_level(travel: u16) -> f32 {
    (f32::from(travel) / FULL_TRAVEL).min(1.0)
}

pub(super) fn output(curve: Curve, level: f32) -> f32 {
    if level <= 0.0 {
        return 0.0;
    }
    let level = level.min(1.0);
    let [dead_zone, knee_low, knee_high, saturation] = curve;
    if saturation.travel <= dead_zone.travel {
        return level;
    }
    let start = f32::from(dead_zone.travel) / POINT_SCALE;
    let end = f32::from(saturation.travel) / POINT_SCALE;
    if level <= start {
        return 0.0;
    }
    let level = if level > end {
        1.0
    } else {
        (level - start) / (end - start)
    };
    let span =
        |point: CurvePoint| f32::from(point.travel.wrapping_sub(dead_zone.travel)) / POINT_SCALE;
    let range = span(saturation);
    let low = (span(knee_low) / range).min(1.0);
    let low_output = scaled(knee_low);
    let shaped = if low > level {
        let first = scaled(dead_zone);
        level.mul_add((low_output - first) / low, first)
    } else {
        let high = (span(knee_high) / range).max(low).min(1.0);
        let high_output = scaled(knee_high);
        if level < high {
            ((high_output - low_output) / (high - low)).mul_add(level - low, low_output)
        } else if level < 1.0 {
            ((scaled(saturation) - high_output) / (1.0 - high)).mul_add(level - high, high_output)
        } else {
            scaled(saturation)
        }
    };
    shaped.min(1.0)
}

pub(super) fn percent(value: f32) -> u8 {
    let rounded = (value * 100.0).round().clamp(0.0, 100.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to 0..=100"
    )]
    let percent = rounded as u8;
    percent
}

pub(super) fn point_value(value: f32) -> u8 {
    let rounded = value.round().clamp(0.0, POINT_SCALE);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to 0..=255"
    )]
    let point = rounded as u8;
    point
}

const fn point(travel: u8, output: u8) -> CurvePoint {
    CurvePoint { travel, output }
}

fn scaled(point: CurvePoint) -> f32 {
    f32::from(point.output) / POINT_SCALE
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::gamepad::Normalization;

    use super::*;

    fn settings(curve: Curve, left_stick_up_bias: Option<u8>) -> Settings {
        Settings {
            curve,
            normalization: Normalization::Circular,
            single_direction_axes: true,
            left_stick_up_bias,
        }
    }

    const IDENTITY: Curve = [point(0, 0), point(85, 85), point(170, 170), point(255, 255)];

    #[test]
    fn every_preset_passes_the_firmware_checks() {
        for preset in &PRESETS {
            assert_eq!(
                check(&settings(preset.curve, None)),
                Ok(()),
                "{}",
                preset.name
            );
        }
    }

    #[test]
    fn presets_are_found_by_their_points() {
        for (index, preset) in PRESETS.iter().enumerate() {
            assert_eq!(preset_of(preset.curve), Some(index));
        }
        assert_eq!(preset_of(IDENTITY), None);
    }

    #[test]
    fn falling_curves_and_curves_without_span_are_refused() {
        let falling = [point(10, 0), point(5, 85), point(170, 170), point(255, 255)];
        assert_eq!(
            check(&settings(falling, None)),
            Err(Problem::CurveNotRising)
        );
        let flat = [point(40, 0), point(40, 85), point(40, 170), point(40, 255)];
        assert_eq!(check(&settings(flat, None)), Err(Problem::CurveWithoutSpan));
    }

    #[test]
    fn a_joystick_angle_above_hundred_percent_is_refused() {
        assert_eq!(check(&settings(IDENTITY, Some(100))), Ok(()));
        assert_eq!(
            check(&settings(IDENTITY, Some(101))),
            Err(Problem::BiasAboveHundred(101))
        );
    }

    #[test]
    fn moved_points_stay_between_their_neighbours() {
        let mut curve = IDENTITY;
        move_point(&mut curve, 1, 200, 40);
        assert_eq!(curve.get(1), Some(&point(170, 40)));
        move_point(&mut curve, 2, 10, 90);
        assert_eq!(curve.get(2), Some(&point(170, 90)));
        move_point(&mut curve, 0, 250, 5);
        assert_eq!(curve.first(), Some(&point(170, 5)));
        move_point(&mut curve, 3, 0, 255);
        assert_eq!(curve.get(3), Some(&point(171, 255)));
        assert_eq!(check(&settings(curve, None)), Ok(()));
    }

    #[test]
    fn the_identity_curve_passes_the_travel_through() {
        for step in 1..=10u8 {
            let level = f32::from(step) / 10.0;
            assert!((output(IDENTITY, level) - level).abs() < 1e-4, "{level}");
        }
        assert!(output(IDENTITY, 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn travel_below_the_dead_zone_gives_no_output_and_above_it_the_anti_dead_zone() {
        let curve = [
            point(51, 51),
            point(102, 102),
            point(153, 153),
            point(204, 255),
        ];
        assert!(output(curve, 0.1).abs() < f32::EPSILON);
        assert!(output(curve, 0.2).abs() < f32::EPSILON);
        assert!((output(curve, 0.201) - 0.2).abs() < 0.01);
        assert!((output(curve, 0.9) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_linear_preset_saturates_before_full_travel() {
        let linear = PRESETS.first().map(|preset| preset.curve);
        assert_eq!(linear.map(|curve| percent(output(curve, 0.995))), Some(100));
        assert_eq!(linear.map(|curve| percent(output(curve, 0.001))), Some(8));
    }

    #[test]
    fn sensor_travel_is_a_fraction_of_full_travel() {
        assert!((travel_level(4095) - 1.0).abs() < f32::EPSILON);
        assert!((travel_level(8000) - 1.0).abs() < f32::EPSILON);
        assert!(travel_level(0).abs() < f32::EPSILON);
    }
}
