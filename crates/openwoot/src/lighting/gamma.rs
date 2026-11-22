pub const GAMMA: [u8; 256] = [
    0, 0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 5, 5,
    5, 5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10, 10, 10, 10, 10, 11, 11, 11, 11, 11, 11,
    12, 12, 13, 13, 13, 13, 14, 14, 15, 15, 16, 16, 17, 17, 18, 18, 18, 18, 19, 19, 20, 20, 21, 21,
    22, 22, 22, 22, 23, 24, 24, 25, 25, 26, 26, 26, 27, 27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 35,
    35, 36, 37, 38, 39, 39, 40, 41, 42, 42, 43, 43, 44, 44, 45, 45, 46, 47, 48, 49, 50, 50, 51, 51,
    52, 52, 53, 54, 54, 55, 55, 56, 56, 57, 57, 58, 58, 59, 59, 60, 60, 61, 61, 62, 62, 63, 64, 65,
    66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 77, 78, 79, 81, 82, 83, 85, 86, 87, 89, 90, 92, 93, 95,
    96, 98, 99, 101, 102, 104, 105, 107, 109, 110, 112, 114, 115, 117, 119, 120, 122, 124, 126,
    127, 129, 131, 133, 135, 137, 138, 140, 142, 144, 146, 148, 150, 152, 154, 156, 158, 160, 162,
    164, 167, 169, 171, 173, 175, 177, 180, 182, 184, 186, 189, 191, 193, 196, 198, 200, 203, 205,
    208, 210, 213, 215, 218, 220, 223, 225, 228, 231, 233, 236, 239, 241, 244, 247, 249, 252, 255,
];

#[must_use]
pub fn gamma(linear: u8) -> u8 {
    GAMMA.get(usize::from(linear)).copied().unwrap_or(u8::MAX)
}

#[must_use]
pub fn output_scale(brightness: u8, fade_level: u8) -> u8 {
    gamma((u16::from(brightness) * u16::from(fade_level) / 255).to_le_bytes()[0])
}

#[must_use]
pub fn output_value(linear: u8, scale: u8) -> u8 {
    super::color::scale_channel(gamma(linear), scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_curve_starts_dark_and_ends_at_full() {
        assert_eq!(GAMMA.len(), 256);
        assert_eq!(gamma(0), 0);
        assert_eq!(gamma(1), 0);
        assert_eq!(gamma(2), 2);
        assert_eq!(gamma(128), 50);
        assert_eq!(gamma(255), 255);
        assert!(GAMMA.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn outputs_apply_the_curve_then_the_brightness() {
        assert_eq!(output_scale(255, 255), 255);
        assert_eq!(output_scale(255, 127), 49);
        assert_eq!(output_scale(0, 255), 0);
        assert_eq!(output_value(255, 255), 255);
        assert_eq!(output_value(128, 255), 50);
        assert_eq!(output_value(255, 49), 49);
        assert_eq!(output_value(0, 255), 0);
    }
}
