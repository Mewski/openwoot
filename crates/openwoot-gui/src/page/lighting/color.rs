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

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Hsv {
    pub(super) hue: f32,
    pub(super) saturation: f32,
    pub(super) value: f32,
}

impl Hsv {
    pub(super) fn of(color: Rgb) -> Self {
        let max = color.red.max(color.green).max(color.blue);
        let min = color.red.min(color.green).min(color.blue);
        let spread = f32::from(max - min);
        let [red, green, blue] = [color.red, color.green, color.blue].map(f32::from);
        let sector = if max == min {
            0.0
        } else if max == color.red {
            (green - blue) / spread
        } else if max == color.green {
            2.0 + (blue - red) / spread
        } else {
            4.0 + (red - green) / spread
        };
        Self {
            hue: (sector / 6.0).rem_euclid(1.0),
            saturation: if max == 0 {
                0.0
            } else {
                spread / f32::from(max)
            },
            value: fraction_of(max),
        }
    }

    pub(super) fn rgb(self) -> Rgb {
        let sector = self.hue.rem_euclid(1.0) * 6.0;
        let chroma = self.value * self.saturation;
        let middle = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
        let (red, green, blue) = match sector {
            turn if turn < 1.0 => (chroma, middle, 0.0),
            turn if turn < 2.0 => (middle, chroma, 0.0),
            turn if turn < 3.0 => (0.0, chroma, middle),
            turn if turn < 4.0 => (0.0, middle, chroma),
            turn if turn < 5.0 => (middle, 0.0, chroma),
            _ => (chroma, 0.0, middle),
        };
        let floor = self.value - chroma;
        Rgb {
            red: channel_of(red + floor),
            green: channel_of(green + floor),
            blue: channel_of(blue + floor),
        }
    }
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
    fn colours_survive_the_trip_through_hsv() {
        for level in 0..=u8::MAX {
            for color in [
                Rgb {
                    red: level,
                    green: 0,
                    blue: u8::MAX,
                },
                Rgb {
                    red: level,
                    green: level,
                    blue: level,
                },
                Rgb {
                    red: 200,
                    green: level,
                    blue: 17,
                },
            ] {
                assert_eq!(Hsv::of(color).rgb(), color);
            }
        }
    }

    #[test]
    fn primaries_sit_a_third_apart() {
        let hues = [
            Rgb {
                red: u8::MAX,
                green: 0,
                blue: 0,
            },
            Rgb {
                red: 0,
                green: u8::MAX,
                blue: 0,
            },
            Rgb {
                red: 0,
                green: 0,
                blue: u8::MAX,
            },
        ]
        .map(|color| Hsv::of(color).hue);
        for (hue, expected) in hues.into_iter().zip([0.0, 1.0 / 3.0, 2.0 / 3.0]) {
            assert!((hue - expected).abs() < 1e-6);
        }
    }
}
