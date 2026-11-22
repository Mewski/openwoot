use libm::fmaf;
use openwoot_protocol::gamepad::Settings;

use super::{normalize_travel, response_curve};

const STRETCH_START: f32 = 0.965;
const STRETCH_END: f32 = 0.992;
const STRETCH_SLOPE: f32 = 1.296_296_8;
const STRETCH_OFFSET: f32 = -0.285_926_4;

pub(super) fn trigger_value(travel: u16, full_scale: u16, settings: Option<&Settings>) -> u16 {
    if travel == 0 {
        return 0;
    }
    let level = normalize_travel(u32::from(travel), settings);
    if level == 0.0 {
        return 0;
    }
    let level = response_curve(level, settings);
    if level >= STRETCH_END {
        return full_scale;
    }
    let stretched = if level > STRETCH_START {
        fmaf(level, STRETCH_SLOPE, STRETCH_OFFSET)
    } else {
        level
    };
    (stretched * f32::from(full_scale)) as u32 as u16
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::gamepad::{CurvePoint, Normalization};

    use super::*;

    #[test]
    fn a_trigger_in_its_dead_zone_is_zero() {
        let raised = Settings {
            curve: [(51, 128), (102, 160), (153, 200), (204, 255)]
                .map(|(travel, output)| CurvePoint { travel, output }),
            normalization: Normalization::Circular,
            single_direction_axes: false,
            left_stick_up_bias: None,
        };
        assert_eq!(trigger_value(0, 255, None), 0);
        assert_eq!(trigger_value(500, 255, Some(&raised)), 0);
        assert_eq!(trigger_value(900, 255, Some(&raised)), 131);
        assert_eq!(trigger_value(4095, 255, Some(&raised)), 255);
    }

    #[test]
    fn the_travel_scales_to_the_full_range() {
        assert_eq!(trigger_value(2048, 255, None), 127);
        assert_eq!(trigger_value(2048, 0x7FFF, None), 16_387);
        assert_eq!(trigger_value(3951, 0x7FFF, None), 31_614);
    }

    #[test]
    fn the_top_end_is_stretched_to_full_scale() {
        assert_eq!(trigger_value(3952, 0x7FFF, None), 31_623);
        assert_eq!(trigger_value(4000, 0x7FFF, None), 32_121);
        assert_eq!(trigger_value(4062, 0x7FFF, None), 32_764);
        assert_eq!(trigger_value(4063, 0x7FFF, None), 0x7FFF);
        assert_eq!(trigger_value(4063, 255, None), 255);
    }
}
