use openwoot_protocol::{
    lightbar::{Background, Effect, Entry, Meter, ProfileIndicator, Range, Target},
    lighting::{Color, Rgb},
};

const MIRROR_GRAY: Rgb = rgb(128, 128, 128);
const LEVEL_MAX: f32 = 255.0;
const METER_FILL: f32 = 0.5;
const LAST_SEGMENT: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Profile {
    pub(super) slot: u8,
    pub(super) count: u8,
    pub(super) color: Rgb,
}

pub(super) fn paint<'entry>(
    entries: impl IntoIterator<Item = &'entry Entry>,
    leds: u8,
    profile: Profile,
) -> Vec<Rgb> {
    let mut colors = vec![Rgb::default(); usize::from(leds)];
    for entry in entries {
        match &entry.effect {
            Effect::Meter(meter) => paint_meter(&mut colors, meter),
            Effect::ProfileIndicator(indicator) => {
                paint_indicator(&mut colors, *indicator, profile);
            }
            Effect::Mirror(mirror) => fill(&mut colors, mirror.range, MIRROR_GRAY),
            Effect::Blink(blink) => {
                if let Target::Lightbar(range) = blink.target {
                    fill(&mut colors, range, blink.to.rgb());
                }
            }
            Effect::Solid(solid) => {
                if let Target::Lightbar(range) = solid.target {
                    fill(&mut colors, range, solid.color.rgb());
                }
            }
            Effect::Custom(custom) => {
                for position in custom.range.first..=custom.range.last {
                    let color = custom
                        .colors
                        .get(usize::from(position))
                        .copied()
                        .unwrap_or_default();
                    set(&mut colors, position, color);
                }
            }
        }
    }
    colors
}

fn contrast(color: Rgb) -> Rgb {
    const YELLOW: Rgb = rgb(0xFF, 0xFF, 0x00);
    const RED: Rgb = rgb(0xFF, 0x00, 0x00);
    const MAGENTA: Rgb = rgb(0xFF, 0x00, 0xFF);
    const BLUE: Rgb = rgb(0x00, 0x00, 0xFF);
    const CYAN: Rgb = rgb(0x00, 0xFF, 0xFF);
    const GREEN: Rgb = rgb(0x00, 0xFF, 0x00);
    const MINT: Rgb = rgb(0x19, 0xDE, 0x3D);
    let red = i32::from(color.red);
    let green = i32::from(color.green);
    let blue = i32::from(color.blue);
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let delta = max - min;
    if max == 0 || low_byte(255 * delta / max) == 0 {
        return YELLOW;
    }
    let hue = if max == red {
        low_byte(43 * (green - blue) / delta)
    } else if max == green {
        low_byte(43 * (blue - red) / delta + 85)
    } else {
        low_byte(43 * (red - green) / delta - 85)
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

fn paint_meter(colors: &mut [Rgb], meter: &Meter) {
    let Range { first, last } = meter.range;
    let count = last.saturating_add(1).saturating_sub(first);
    let length = f32::from(count) * METER_FILL;
    let lit = meter.color.rgb();
    let start = meter.gradient_start.map_or(lit, Color::rgb);
    for index in 0..count {
        let position = if meter.reverse {
            last.saturating_sub(index)
        } else {
            first.saturating_add(index)
        };
        let at = f32::from(index);
        let under = meter
            .background
            .map(Color::rgb)
            .or_else(|| colors.get(usize::from(position)).copied())
            .unwrap_or_default();
        let color = if length > at {
            let shade = if meter.gradient_start.is_some() {
                blend(start, lit, at / f32::from(count))
            } else {
                lit
            };
            blend(under, shade, (length - at).min(1.0))
        } else {
            under
        };
        set(colors, position, color);
    }
}

fn paint_indicator(colors: &mut [Rgb], indicator: ProfileIndicator, profile: Profile) {
    let Range { first, last } = indicator.range;
    if let Some(background) = background_of(indicator.background, profile.color) {
        fill(colors, indicator.range, background);
    }
    let count = last.saturating_add(1).saturating_sub(first);
    let by_segments = count.checked_div(indicator.segments).unwrap_or(0);
    let by_profiles = count.checked_div(profile.count.max(1)).unwrap_or(0);
    let fitted = by_segments.min(by_profiles);
    let segment = profile.slot.min(LAST_SEGMENT);
    let start = first.saturating_add(fitted.saturating_mul(segment));
    for position in start..start.saturating_add(fitted.max(1)) {
        set(colors, position, profile.color);
    }
}

fn background_of(background: Background, profile: Rgb) -> Option<Rgb> {
    match background {
        Background::ProfileColor => Some(profile),
        Background::Transparent => None,
        Background::Fixed(color) => Some(color.rgb()),
        Background::Contrast => Some(contrast(profile)),
        Background::DimmedProfileColor => {
            Some(rgb(profile.red >> 1, profile.green >> 1, profile.blue >> 1))
        }
    }
}

fn fill(colors: &mut [Rgb], range: Range, color: Rgb) {
    for position in range.first..=range.last {
        set(colors, position, color);
    }
}

fn set(colors: &mut [Rgb], position: u8, color: Rgb) {
    if let Some(slot) = colors.get_mut(usize::from(position)) {
        *slot = color;
    }
}

fn blend(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |from: u8, to: u8| {
        let value = (f32::from(from) + (f32::from(to) - f32::from(from)) * amount)
            .round()
            .clamp(0.0, LEVEL_MAX);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "the value is rounded and clamped to 0..=255"
        )]
        let channel = value as u8;
        channel
    };
    rgb(
        mix(from.red, to.red),
        mix(from.green, to.green),
        mix(from.blue, to.blue),
    )
}

const fn rgb(red: u8, green: u8, blue: u8) -> Rgb {
    Rgb { red, green, blue }
}

const fn low_byte(value: i32) -> u8 {
    value.to_le_bytes()[0]
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        heapless,
        lightbar::{Builtin, Custom, Signal, Solid},
    };

    use super::*;

    const RED: Rgb = rgb(255, 0, 0);
    const BLUE: Rgb = rgb(0, 0, 255);
    const BLACK: Rgb = rgb(0, 0, 0);
    const PROFILE: Profile = Profile {
        slot: 1,
        count: 2,
        color: BLUE,
    };

    fn plain(effect: Effect) -> Entry {
        Entry {
            condition: None,
            effect,
        }
    }

    fn solid(color: Rgb, first: u8, last: u8) -> Entry {
        plain(Effect::Solid(Solid {
            color: color.opaque(),
            target: Target::Lightbar(Range { first, last }),
        }))
    }

    #[test]
    fn later_entries_paint_over_earlier_ones() {
        let entries = [solid(RED, 0, 9), solid(BLUE, 2, 3)];
        let colors = paint(&entries, 10, PROFILE);
        assert_eq!(colors.len(), 10);
        assert_eq!(colors[0], RED);
        assert_eq!(colors[2], BLUE);
        assert_eq!(colors[3], BLUE);
        assert_eq!(colors[4], RED);
    }

    #[test]
    fn custom_colours_belong_to_their_led_positions() {
        let mut list = heapless::Vec::new();
        for color in [RED, BLUE, RED, BLUE] {
            list.push(color).ok();
        }
        let entries = [plain(Effect::Custom(Custom {
            colors: list,
            range: Range { first: 2, last: 5 },
        }))];
        let colors = paint(&entries, 6, PROFILE);
        assert_eq!(colors, [BLACK, BLACK, RED, BLUE, BLACK, BLACK]);
    }

    #[test]
    fn the_profile_indicator_marks_the_slot_of_the_active_profile() {
        let entries = [plain(Effect::ProfileIndicator(ProfileIndicator {
            range: Range { first: 0, last: 9 },
            background: Background::Fixed(Color::TRANSPARENT),
            segments: 1,
        }))];
        let colors = paint(&entries, 10, PROFILE);
        assert_eq!(colors[..5], [BLACK; 5]);
        assert_eq!(colors[5..], [BLUE; 5]);
        let five = [plain(Effect::ProfileIndicator(ProfileIndicator {
            range: Range { first: 0, last: 9 },
            background: Background::Transparent,
            segments: 5,
        }))];
        let colors = paint(&five, 10, PROFILE);
        assert_eq!(colors[2..4], [BLUE; 2]);
        assert_eq!(colors[4], BLACK);
    }

    #[test]
    fn a_meter_shows_half_full_over_its_background() {
        let entries = [plain(Effect::Meter(Meter {
            signal: Signal::Builtin(Builtin::Brightness),
            maximum: 100.0,
            color: RED.opaque(),
            background: Some(BLUE.opaque()),
            gradient_start: None,
            range: Range { first: 0, last: 3 },
            reverse: true,
        }))];
        let colors = paint(&entries, 4, PROFILE);
        assert_eq!(colors, [BLUE, BLUE, RED, RED]);
    }

    #[test]
    fn contrast_follows_the_firmware_table() {
        let yellow = rgb(0xFF, 0xFF, 0x00);
        assert_eq!(contrast(BLACK), yellow);
        assert_eq!(contrast(RED), yellow);
        assert_eq!(contrast(BLUE), rgb(0x00, 0xFF, 0x00));
        assert_eq!(contrast(rgb(0, 255, 0)), rgb(0x00, 0xFF, 0xFF));
        assert_eq!(contrast(rgb(255, 100, 0)), RED);
    }
}
