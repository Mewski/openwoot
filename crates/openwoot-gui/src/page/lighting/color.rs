use gpui_kit::{Hsla, Rgba};
use openwoot_protocol::lighting::{Color, Rgb};

pub(crate) const BLACK: Rgb = Rgb {
    red: 0,
    green: 0,
    blue: 0,
};
pub(crate) const WHITE: Rgb = Rgb {
    red: u8::MAX,
    green: u8::MAX,
    blue: u8::MAX,
};

const CHANNEL_MAX: f32 = 255.0;

pub(crate) fn hsla_of(color: Rgb) -> Hsla {
    Hsla::from(Rgba {
        r: fraction_of(color.red),
        g: fraction_of(color.green),
        b: fraction_of(color.blue),
        a: 1.0,
    })
}

pub(crate) fn rgb_of(color: Hsla) -> Rgb {
    let rgba = color.to_rgb();
    Rgb {
        red: channel_of(rgba.r),
        green: channel_of(rgba.g),
        blue: channel_of(rgba.b),
    }
}

pub(crate) fn opaque_of(color: Hsla) -> Color {
    rgb_of(color).opaque()
}

pub(crate) fn hex(color: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", color.red, color.green, color.blue)
}

fn fraction_of(channel: u8) -> f32 {
    f32::from(channel) / CHANNEL_MAX
}

fn channel_of(fraction: f32) -> u8 {
    let scaled = (fraction.clamp(0.0, 1.0) * CHANNEL_MAX).round();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to 0..=255"
    )]
    let channel = scaled as u8;
    channel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_survive_the_trip_through_the_colour_picker() {
        let samples = [
            BLACK,
            WHITE,
            Rgb {
                red: 0,
                green: 119,
                blue: 255,
            },
            Rgb {
                red: 255,
                green: 40,
                blue: 40,
            },
            Rgb {
                red: 1,
                green: 2,
                blue: 3,
            },
            Rgb {
                red: 123,
                green: 0,
                blue: 255,
            },
        ];
        for color in samples {
            assert_eq!(rgb_of(hsla_of(color)), color);
        }
        for channel in 0..=u8::MAX {
            let gray = Rgb {
                red: channel,
                green: channel,
                blue: channel,
            };
            assert_eq!(rgb_of(hsla_of(gray)), gray);
            let red = Rgb {
                red: channel,
                green: 0,
                blue: 0,
            };
            assert_eq!(rgb_of(hsla_of(red)), red);
        }
    }

    #[test]
    fn an_opaque_colour_has_full_alpha() {
        let color = opaque_of(hsla_of(WHITE));
        assert_eq!(color.alpha, Color::OPAQUE);
        assert_eq!(color.rgb(), WHITE);
    }

    #[test]
    fn hex_shows_two_digits_per_channel() {
        assert_eq!(
            hex(Rgb {
                red: 0,
                green: 119,
                blue: 255
            }),
            "#0077FF"
        );
    }
}
