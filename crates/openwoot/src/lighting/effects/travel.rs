use crate::lighting::{
    input::{KeyTravel, TRAVEL_FULL},
    truncate::{low_8, low_16},
};

const LEVEL_DEAD_ZONE: u8 = 5;
const METER_DEAD_ZONE: u16 = 80;

pub(super) fn travel_level_8(gamepad_curve: bool, travel: u16, input: &impl KeyTravel) -> u8 {
    if !gamepad_curve || !input.gamepad_mode() {
        let level = linear_level(travel);
        return if level > LEVEL_DEAD_ZONE { level } else { 0 };
    }
    if travel == 0 {
        return 0;
    }
    float_to_byte(input.response_curve(travel) * 255.0)
}

pub(super) fn travel_level_12(gamepad_curve: bool, travel: u16, input: &impl KeyTravel) -> u16 {
    if !gamepad_curve || !input.gamepad_mode() {
        return if travel > METER_DEAD_ZONE { travel } else { 0 };
    }
    if travel == 0 {
        return 0;
    }
    let curve = input.response_curve(travel);
    if curve.is_nan() || curve <= 0.0 {
        0
    } else if curve >= 1.0 {
        TRAVEL_FULL
    } else {
        low_16(float_to_u32(curve * 4095.0))
    }
}

fn linear_level(travel: u16) -> u8 {
    low_8(255 * u32::from(travel) / u32::from(TRAVEL_FULL))
}

fn float_to_byte(value: f32) -> u8 {
    low_8(float_to_u32(value))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "levels are converted to integers by truncation, negative values give 0"
)]
fn float_to_u32(value: f32) -> u32 {
    value as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Gamepad;

    impl KeyTravel for Gamepad {
        fn gamepad_mode(&self) -> bool {
            true
        }

        fn response_curve(&self, travel: u16) -> f32 {
            if travel < 400 { 0.0 } else { 0.5 }
        }
    }

    #[test]
    fn levels_ignore_the_dead_zones_without_the_gamepad_curve() {
        assert_eq!(travel_level_8(false, 0, &()), 0);
        assert_eq!(travel_level_8(false, 96, &()), 0);
        assert_eq!(travel_level_8(false, 112, &()), 6);
        assert_eq!(travel_level_8(true, 4095, &()), 255);
        assert_eq!(travel_level_12(false, 80, &()), 0);
        assert_eq!(travel_level_12(false, 81, &()), 81);
    }

    #[test]
    fn levels_follow_the_gamepad_curve_in_gamepad_mode() {
        assert_eq!(travel_level_8(true, 300, &Gamepad), 0);
        assert_eq!(travel_level_8(true, 1000, &Gamepad), 127);
        assert_eq!(travel_level_8(false, 1000, &Gamepad), 62);
        assert_eq!(travel_level_12(true, 300, &Gamepad), 0);
        assert_eq!(travel_level_12(true, 1000, &Gamepad), 2047);
    }
}
