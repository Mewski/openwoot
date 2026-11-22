use libm::{fmaf, sqrtf};
use openwoot_protocol::gamepad::{Normalization, Settings};

use super::response_curve;

const EVEN_WEIGHT: f32 = 0.5;
const PERCENT: f32 = 100.0;

#[must_use]
pub fn shape_stick(
    x: f32,
    y: f32,
    left_stick: bool,
    min_magnitude: f32,
    settings: Option<&Settings>,
) -> (f32, f32) {
    let largest = x.abs().max(y.abs());
    if largest < min_magnitude {
        return (0.0, 0.0);
    }
    let magnitude = if largest == 0.0 {
        0.0
    } else {
        response_curve(largest, settings)
    };
    let up_bias = settings
        .and_then(|settings| settings.left_stick_up_bias)
        .filter(|_| left_stick && y < 0.0);
    let (weight_x, weight_y) = match up_bias {
        Some(percent) => {
            let weight_x = f32::from(percent) / PERCENT;
            (weight_x, 1.0 - weight_x)
        }
        None => (EVEN_WEIGHT, EVEN_WEIGHT),
    };
    let weighted_x = x * weight_x;
    let weighted_y = y * weight_y;
    let length = match settings.map(|settings| settings.normalization) {
        Some(Normalization::Square) => weighted_x.abs().max(weighted_y.abs()),
        _ => sqrtf(fmaf(weighted_x, weighted_x, weighted_y * weighted_y)),
    };
    let (unit_x, unit_y) = if length == 0.0 {
        (0.0, 0.0)
    } else {
        (weighted_x / length, weighted_y / length)
    };
    (unit_x * magnitude, unit_y * magnitude)
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::gamepad::CurvePoint;

    use super::*;

    fn settings(normalization: Normalization, left_stick_up_bias: Option<u8>) -> Settings {
        Settings {
            curve: [(0, 0), (85, 85), (170, 170), (255, 255)]
                .map(|(travel, output)| CurvePoint { travel, output }),
            normalization,
            single_direction_axes: false,
            left_stick_up_bias,
        }
    }

    #[test]
    fn a_deflection_below_the_minimum_magnitude_centres_the_stick() {
        assert_eq!(
            shape_stick(1e-5, -2e-5, true, 1.0 / 32767.0, None),
            (0.0, 0.0)
        );
        assert_eq!(shape_stick(0.0, 0.0, false, 0.0, None), (0.0, 0.0));
    }

    #[test]
    fn the_direction_keeps_the_larger_component_as_magnitude() {
        assert_eq!(shape_stick(1.0, 0.0, false, 0.0, None), (1.0, 0.0));
        let (x, y) = shape_stick(1.0, 1.0, false, 0.0, None);
        assert_eq!(x, y);
        assert!((x - core::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
        let square = settings(Normalization::Square, None);
        assert_eq!(
            shape_stick(1.0, -1.0, false, 0.0, Some(&square)),
            (1.0, -1.0)
        );
        assert_eq!(
            shape_stick(-0.5, 0.25, false, 0.0, Some(&square)),
            (-0.5, 0.25)
        );
    }

    #[test]
    fn the_up_bias_weights_the_left_stick_pointing_up() {
        let biased = settings(Normalization::Square, Some(25));
        assert_eq!(
            shape_stick(1.0, -1.0, true, 0.0, Some(&biased)),
            (1.0 / 3.0, -1.0)
        );
        assert_eq!(shape_stick(1.0, 1.0, true, 0.0, Some(&biased)), (1.0, 1.0));
        assert_eq!(
            shape_stick(1.0, -1.0, false, 0.0, Some(&biased)),
            (1.0, -1.0)
        );
    }
}
