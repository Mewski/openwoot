use openwoot_protocol::{lightbar::Background, lighting::Rgb};

use crate::lighting::color::{opaque, rgb};

const YELLOW: Rgb = rgb(0xFF, 0xFF, 0x00);
const RED: Rgb = rgb(0xFF, 0x00, 0x00);
const MAGENTA: Rgb = rgb(0xFF, 0x00, 0xFF);
const BLUE: Rgb = rgb(0x00, 0x00, 0xFF);
const CYAN: Rgb = rgb(0x00, 0xFF, 0xFF);
const GREEN: Rgb = rgb(0x00, 0xFF, 0x00);
const MINT: Rgb = rgb(0x19, 0xDE, 0x3D);

pub(super) fn background_of(background: Background, profile_color: Rgb) -> Option<Rgb> {
    match background {
        Background::ProfileColor => Some(profile_color),
        Background::Transparent => None,
        Background::Fixed(color) => Some(opaque(color)),
        Background::Contrast => Some(contrast(profile_color)),
        Background::DimmedProfileColor => Some(rgb(
            profile_color.red >> 1,
            profile_color.green >> 1,
            profile_color.blue >> 1,
        )),
    }
}

fn contrast(color: Rgb) -> Rgb {
    let red = i32::from(color.red);
    let green = i32::from(color.green);
    let blue = i32::from(color.blue);
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    if max == 0 {
        return YELLOW;
    }
    let delta = max - min;
    if low_byte(255 * delta / max) == 0 {
        return YELLOW;
    }
    let hue = if max == red {
        low_byte(sixths(green - blue, delta))
    } else if max == green {
        low_byte(sixths(blue - red, delta) + 85)
    } else {
        low_byte(sixths(red - green, delta) - 85)
    };
    match hue {
        0..=14 | 199..=240 => YELLOW,
        15..=28 | 107..=120 | 185..=198 => RED,
        29..=42 | 121..=141 => MAGENTA,
        43..=56 => BLUE,
        57..=106 => CYAN,
        142..=184 => GREEN,
        241..=255 => MINT,
    }
}

fn sixths(difference: i32, delta: i32) -> i32 {
    (43 * difference).checked_div(delta).unwrap_or_default()
}

const fn low_byte(value: i32) -> u8 {
    value.to_le_bytes()[0]
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::lighting::Color;

    use super::*;

    #[test]
    fn the_contrast_colour_follows_the_hue_of_the_profile_colour() {
        let contrast_of = |color| background_of(Background::Contrast, color);
        assert_eq!(contrast_of(rgb(0, 0, 0)), Some(YELLOW));
        assert_eq!(contrast_of(rgb(90, 90, 90)), Some(YELLOW));
        assert_eq!(contrast_of(rgb(255, 0, 0)), Some(YELLOW));
        assert_eq!(contrast_of(rgb(255, 0x28, 0x28)), Some(YELLOW));
        assert_eq!(contrast_of(rgb(0, 0, 255)), Some(GREEN));
        assert_eq!(contrast_of(rgb(0, 255, 0)), Some(CYAN));
        assert_eq!(contrast_of(rgb(255, 0, 255)), Some(YELLOW));
        assert_eq!(contrast_of(rgb(255, 100, 0)), Some(RED));
        assert_eq!(contrast_of(rgb(255, 0, 40)), Some(MINT));
    }

    #[test]
    fn the_other_backgrounds_derive_from_the_profile_colour_or_stand_alone() {
        let color = rgb(201, 100, 3);
        assert_eq!(background_of(Background::ProfileColor, color), Some(color));
        assert_eq!(background_of(Background::Transparent, color), None);
        assert_eq!(
            background_of(Background::DimmedProfileColor, color),
            Some(rgb(100, 50, 1))
        );
        assert_eq!(
            background_of(
                Background::Fixed(Color {
                    red: 1,
                    green: 2,
                    blue: 3,
                    alpha: 0
                }),
                color
            ),
            Some(rgb(1, 2, 3))
        );
    }
}
