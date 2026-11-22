use super::config::{Color, Rgb};

pub const BLACK: Rgb = Rgb {
    red: 0,
    green: 0,
    blue: 0,
};

const PALETTE: [Rgb; 7] = [
    rgb(0xFF, 0x00, 0x00),
    rgb(0x00, 0xFF, 0x00),
    rgb(0x00, 0x00, 0xFF),
    rgb(0xFF, 0xFF, 0x00),
    rgb(0xFF, 0x00, 0xFF),
    rgb(0x00, 0xFF, 0xFF),
    rgb(0xFF, 0x80, 0x00),
];

#[must_use]
pub const fn rgb(red: u8, green: u8, blue: u8) -> Rgb {
    Rgb { red, green, blue }
}

#[must_use]
pub const fn from_rgb565(value: u16) -> Rgb {
    let [high, low] = value.to_be_bytes();
    let red = high & 0xF8;
    let green = (value >> 3).to_le_bytes()[0] & 0xFC;
    let blue = low << 3;
    rgb(
        fill_low_bits(red, 7),
        fill_low_bits(green, 3),
        fill_low_bits(blue, 7),
    )
}

#[must_use]
pub const fn scale(color: Rgb, factor: u8) -> Rgb {
    rgb(
        scale_channel(color.red, factor),
        scale_channel(color.green, factor),
        scale_channel(color.blue, factor),
    )
}

pub(super) const fn opaque(color: Color) -> Rgb {
    rgb(color.red, color.green, color.blue)
}

pub(super) fn blend(from: Rgb, to: Rgb, amount: u8) -> Rgb {
    match amount {
        0 => from,
        u8::MAX => to,
        _ => rgb(
            blend_channel(from.red, to.red, amount),
            blend_channel(from.green, to.green, amount),
            blend_channel(from.blue, to.blue, amount),
        ),
    }
}

pub(super) const fn scale_channel(channel: u8, factor: u8) -> u8 {
    let factor = factor as u16;
    ((factor + channel as u16 * factor) >> 8) as u8
}

pub(super) const fn mix_level(base_level: u8, level: u8) -> u8 {
    (((base_level as u16) << 8).wrapping_add((255 - base_level as u16) * level as u16) >> 8) as u8
}

pub(super) fn palette_color(index: u8) -> Rgb {
    usize::from(index)
        .checked_sub(1)
        .and_then(|index| PALETTE.get(index))
        .copied()
        .unwrap_or(BLACK)
}

const fn fill_low_bits(channel: u8, bits: u8) -> u8 {
    if channel == 0 { 0 } else { channel + bits }
}

fn blend_channel(from: u8, to: u8, amount: u8) -> u8 {
    let mixed = u16::from(to) * u16::from(amount) + u16::from(255 - amount) * u16::from(from);
    mixed.to_be_bytes()[0]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb565_channels_fill_their_low_bits_unless_zero() {
        assert_eq!(from_rgb565(0xFFFF), rgb(255, 255, 255));
        assert_eq!(from_rgb565(0x0000), BLACK);
        assert_eq!(from_rgb565(0xF800), rgb(255, 0, 0));
        assert_eq!(from_rgb565(0x0820), rgb(15, 7, 0));
        assert_eq!(from_rgb565(0x0001), rgb(0, 0, 15));
    }

    #[test]
    fn blending_keeps_the_ends_and_truncates_between() {
        let from = rgb(10, 200, 0);
        let to = rgb(250, 0, 100);
        assert_eq!(blend(from, to, 0), from);
        assert_eq!(blend(from, to, 255), to);
        assert_eq!(blend(from, to, 128), rgb(129, 99, 50));
    }

    #[test]
    fn scaling_by_full_keeps_full_and_by_zero_gives_zero() {
        assert_eq!(scale_channel(255, 255), 255);
        assert_eq!(scale_channel(0, 255), 0);
        assert_eq!(scale_channel(255, 0), 0);
        assert_eq!(scale_channel(100, 128), 50);
        assert_eq!(mix_level(80, 0), 80);
        assert_eq!(mix_level(80, 255), 254);
        assert_eq!(mix_level(0, 255), 254);
    }

    #[test]
    fn palette_indices_start_at_one() {
        assert_eq!(palette_color(1), rgb(255, 0, 0));
        assert_eq!(palette_color(7), rgb(255, 128, 0));
        assert_eq!(palette_color(0), BLACK);
        assert_eq!(palette_color(8), BLACK);
    }
}
