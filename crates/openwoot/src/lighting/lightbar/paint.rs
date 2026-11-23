use heapless::Vec;
use openwoot_protocol::{
    lightbar::{Blink, Custom, KeyRect, Meter, Mirror, ProfileIndicator, Range, Target, Waveform},
    lighting::Rgb,
};

use super::state::IndicatorState;
use crate::lighting::{
    canvas::Canvas,
    color::{BLACK, blend, opaque},
    leds::Led,
    overlay::MAX_LIGHTBAR_LEDS,
    truncate::{low_8, low_16},
};

#[derive(Clone, Copy)]
pub(super) struct Strip<'a> {
    pub(super) leds: &'a [Led],
    pub(super) columns: u8,
}

impl Strip<'_> {
    fn led(self, position: u8) -> Option<Led> {
        self.leds.get(usize::from(position)).copied()
    }

    pub(super) fn clear(self, canvas: &mut impl Canvas) {
        for led in self.leds {
            canvas.set_led(*led, BLACK);
        }
    }

    pub(super) fn fill(self, canvas: &mut impl Canvas, target: Target, color: Rgb) {
        match target {
            Target::Lightbar(range) => self.fill_range(canvas, range, color),
            Target::Keys(rect) => fill_keys(canvas, rect, color),
        }
    }

    fn fill_range(self, canvas: &mut impl Canvas, range: Range, color: Rgb) {
        for position in range.first..=range.last {
            if let Some(led) = self.led(position) {
                canvas.set_led(led, color);
            }
        }
    }

    pub(super) fn meter(self, canvas: &mut impl Canvas, meter: &Meter, value: f32) {
        let lit = opaque(meter.color);
        let start = meter.gradient_start.map_or(lit, opaque);
        let background = meter.background.map(opaque);
        let range = meter.range;
        let count = range.last.wrapping_add(1).wrapping_sub(range.first);
        let full = f32::from(count);
        let length = value / meter.maximum * full;
        let length = if length > full { full } else { length };
        for index in 0..count {
            let position = if meter.reverse {
                range.last.wrapping_sub(index)
            } else {
                range.first.wrapping_add(index)
            };
            let Some(led) = self.led(position) else {
                continue;
            };
            let at = f32::from(index);
            let level = if low_8(whole(length)) == index {
                low_8(whole((length - at) * 255.0))
            } else if length > at {
                u8::MAX
            } else {
                0
            };
            if length > at {
                let color = if meter.gradient_start.is_some() {
                    blend(start, lit, low_8(255 * u32::from(index) / u32::from(count)))
                } else {
                    lit
                };
                if let Some(background) = background {
                    canvas.set_led(led, blend(background, color, level));
                } else if let Some(shown) = canvas.led(led) {
                    let mixed = blend(shown, canvas.stored(color), level);
                    canvas.store_led(led, mixed);
                }
            } else if let Some(background) = background {
                canvas.set_led(led, background);
            }
        }
    }

    pub(super) fn indicator(
        self,
        canvas: &mut impl Canvas,
        indicator: ProfileIndicator,
        view: &IndicatorState,
    ) {
        let Range { first, last } = indicator.range;
        let profile = whole(view.profile);
        let (segment, namespace) = match low_8(profile) {
            0 => (0, 0),
            number => ((number - 1).min(4), low_16(profile) >> 8),
        };
        if let Some(background) = view.background
            && first <= last
        {
            self.fill_range(canvas, indicator.range, background);
        }
        let count = last.wrapping_add(1).wrapping_sub(first);
        let (start, length) = if namespace == 1 {
            let length = count.min(2);
            (last.wrapping_add(1).wrapping_sub(length), length)
        } else {
            let by_segments = f32::from(count.checked_div(indicator.segments).unwrap_or(0));
            let by_profiles = f32::from(count) / view.count;
            let length = low_8(whole(if by_profiles < by_segments {
                by_profiles
            } else {
                by_segments
            }));
            let start = low_8(u32::from(first) + u32::from(length) * u32::from(segment));
            (start, length.max(1))
        };
        for position in u32::from(start)..u32::from(start) + u32::from(length) {
            if let Some(led) = u8::try_from(position)
                .ok()
                .and_then(|position| self.led(position))
            {
                canvas.set_led(led, view.color);
            }
        }
    }

    pub(super) fn mirror(self, canvas: &mut impl Canvas, mirror: Mirror) {
        let source = mirror.source;
        let samples = if source.first.row == source.last.row {
            source
                .last
                .column
                .wrapping_add(1)
                .wrapping_sub(source.first.column)
        } else {
            source
                .last
                .row
                .wrapping_add(1)
                .wrapping_sub(source.first.row)
        };
        let columns = usize::from(self.columns);
        let sample = |index: u8| -> Rgb {
            let flat = columns * usize::from(source.first.row)
                + usize::from(source.first.column.wrapping_add(index));
            let key = flat.checked_div(columns).zip(flat.checked_rem(columns));
            match key {
                Some((row, column)) if index < samples => canvas.key(row, column).unwrap_or(BLACK),
                _ => BLACK,
            }
        };
        let range = mirror.range;
        let count = range.last.wrapping_add(1).wrapping_sub(range.first);
        if count == 0 {
            return;
        }
        let sample_span = f32::from(i16::from(samples) - 1);
        let target_span = f32::from(i16::from(count) - 1);
        let mut colors = Vec::<(Led, Rgb), MAX_LIGHTBAR_LEDS>::new();
        for index in 0..count {
            let Some(led) = self.led(range.first.wrapping_add(index)) else {
                break;
            };
            let at = f32::from(index) / target_span * sample_span;
            let first = low_8(whole(at));
            let fraction = at - f32::from(first);
            let weight = low_8(whole_wide(f64::from(fraction) * 255.0));
            let color = blend(sample(first), sample(first.wrapping_add(1)), weight);
            if colors.push((led, color)).is_err() {
                break;
            }
        }
        for (led, color) in colors {
            canvas.store_led(led, color);
        }
    }

    pub(super) fn blink(
        self,
        canvas: &mut impl Canvas,
        blink: &Blink,
        phase_ms: &mut u16,
        elapsed_ms: u16,
        restart: bool,
    ) {
        let period = blink.period_ms;
        let phase = if restart {
            0
        } else {
            phase_ms
                .wrapping_add(elapsed_ms)
                .checked_rem(period)
                .unwrap_or(0)
        };
        *phase_ms = phase;
        let mut cycle = f32::from(phase) / f32::from(period);
        let weight = match blink.waveform {
            Waveform::Square => {
                if (0.25..0.75).contains(&cycle) {
                    u8::MAX
                } else {
                    0
                }
            }
            Waveform::Triangle => {
                if cycle > 0.5 {
                    cycle = 1.0 - cycle;
                }
                low_8(whole((cycle + cycle) * 255.0))
            }
        };
        let color = blend(opaque(blink.from), opaque(blink.to), weight);
        self.fill(canvas, blink.target, color);
    }

    pub(super) fn custom(self, canvas: &mut impl Canvas, custom: &Custom) {
        for position in custom.range.first..=custom.range.last {
            if let Some(led) = self.led(position) {
                let color = custom
                    .colors
                    .get(usize::from(position))
                    .copied()
                    .unwrap_or(BLACK);
                canvas.set_led(led, color);
            }
        }
    }
}

fn fill_keys(canvas: &mut impl Canvas, rect: KeyRect, color: Rgb) {
    for row in rect.first.row..=rect.last.row {
        for column in rect.first.column..=rect.last.column {
            canvas.set_key(usize::from(row), usize::from(column), color);
        }
    }
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "values are converted to integers by truncation as the microcontroller converts them: negative values and NaN give 0, large values the maximum"
)]
fn whole(value: f32) -> u32 {
    value as u32
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "values are converted to integers by truncation as the microcontroller converts them: negative values and NaN give 0, large values the maximum"
)]
fn whole_wide(value: f64) -> u32 {
    value as u32
}
