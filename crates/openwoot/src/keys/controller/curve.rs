use libm::fmaf;
use openwoot_protocol::gamepad::{CurvePoint, Settings};

const FULL_TRAVEL: f32 = 4095.0;
const POINT_SCALE: f32 = 255.0;

#[must_use]
pub fn normalize_travel(travel: u32, settings: Option<&Settings>) -> f32 {
    let level = (travel as f32 / FULL_TRAVEL).min(1.0);
    let Some(&Settings {
        curve: [dead_zone, .., saturation],
        ..
    }) = settings
    else {
        return level;
    };
    let start = f32::from(dead_zone.travel) / POINT_SCALE;
    if level < start {
        return 0.0;
    }
    let end = f32::from(saturation.travel) / POINT_SCALE;
    if level > end {
        return 1.0;
    }
    (level - start) / (end - start)
}

#[must_use]
pub fn response_curve(level: f32, settings: Option<&Settings>) -> f32 {
    let Some(&Settings {
        curve: [dead_zone, knee_low, knee_high, saturation],
        ..
    }) = settings
    else {
        return level;
    };
    let span =
        |point: CurvePoint| f32::from(point.travel.wrapping_sub(dead_zone.travel)) / POINT_SCALE;
    let range = span(saturation);
    let low = span(knee_low) / range;
    let low_output = output(knee_low);
    let shaped = if low > level {
        let start = output(dead_zone);
        fmaf(level, (low_output - start) / low, start)
    } else {
        let high = span(knee_high) / range;
        let high_output = output(knee_high);
        if level < high {
            fmaf(
                (high_output - low_output) / (high - low),
                level - low,
                low_output,
            )
        } else if level < 1.0 {
            fmaf(
                (output(saturation) - high_output) / (1.0 - high),
                level - high,
                high_output,
            )
        } else {
            output(saturation)
        }
    };
    shaped.min(1.0)
}

fn output(point: CurvePoint) -> f32 {
    f32::from(point.output) / POINT_SCALE
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::gamepad::Normalization;

    use super::*;

    fn settings(points: [(u8, u8); 4]) -> Settings {
        Settings {
            curve: points.map(|(travel, output)| CurvePoint { travel, output }),
            normalization: Normalization::Circular,
            single_direction_axes: false,
            left_stick_up_bias: None,
        }
    }

    #[test]
    fn without_settings_the_travel_is_a_fraction_of_full_travel() {
        assert_eq!(normalize_travel(0, None), 0.0);
        assert_eq!(normalize_travel(4095, None), 1.0);
        assert_eq!(normalize_travel(65_535, None), 1.0);
        assert_eq!(normalize_travel(2048, None), 2048.0 / 4095.0);
        assert_eq!(response_curve(0.25, None), 0.25);
    }

    #[test]
    fn the_travel_is_rescaled_to_the_input_range_of_the_curve() {
        let curve = settings([(51, 0), (102, 102), (153, 153), (204, 255)]);
        assert_eq!(normalize_travel(800, Some(&curve)), 0.0);
        assert_eq!(normalize_travel(3300, Some(&curve)), 1.0);
        let middle = normalize_travel(2048, Some(&curve));
        assert!((middle - 0.5).abs() < 1e-3);
    }

    #[test]
    fn the_curve_is_linear_between_its_points_and_never_above_one() {
        let curve = settings([(0, 0), (51, 102), (204, 153), (255, 255)]);
        assert_eq!(response_curve(0.1, Some(&curve)), 0.2);
        assert_eq!(response_curve(0.5, Some(&curve)), 0.5);
        assert_eq!(response_curve(1.0, Some(&curve)), 1.0);
        assert_eq!(response_curve(2.0, Some(&curve)), 1.0);
        let falling = settings([(0, 255), (128, 0), (192, 0), (255, 0)]);
        assert_eq!(response_curve(0.25, Some(&falling)), 0.501_953_1);
        assert_eq!(response_curve(-0.5, Some(&falling)), 1.0);
    }

    #[test]
    fn a_curve_without_input_range_divides_by_zero() {
        let flat = settings([(0, 0); 4]);
        assert!(normalize_travel(0, Some(&flat)).is_nan());
        assert_eq!(normalize_travel(1, Some(&flat)), 1.0);
        assert_eq!(response_curve(0.5, Some(&flat)), 1.0);
        assert_eq!(response_curve(f32::NAN, Some(&flat)), 0.0);
    }
}
