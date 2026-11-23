use openwoot_protocol::lighting::{ReactiveColoring, ReactiveFade, Rgb};

use super::{cells::Cells, press_color::pick_press_color};
use crate::{
    keys::Key,
    lighting::{
        canvas::Canvas,
        color::{mix_level, opaque, palette_color, rgb, scale},
        leds::Variant,
        truncate::{low_8, low_16},
    },
};

const STEP_MS_MAX: u8 = 40;
const PALETTE_STEP_FACTOR: u32 = 8;
const PRESSED: u8 = 251;
const LEVEL_MAX: u32 = 250;
const CROSSFADE_LEVEL: u32 = 100;
const PALETTE_COLOR_MASK: u8 = 0x07;
const PALETTE_LEVEL_MASK: u8 = 0xF8;
const PALETTE_LEVEL_SHIFT: u8 = 3;
const PALETTE_LEVEL_MAX: u32 = 30;
const PALETTE_PRESSED: u8 = 0xF8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State<const ROWS: usize, const COLUMNS: usize> {
    config: ReactiveFade,
    cells: Cells<ROWS, COLUMNS>,
    last_color: u8,
    active: bool,
    timer_ms: u16,
}

impl<const ROWS: usize, const COLUMNS: usize> State<ROWS, COLUMNS> {
    pub(super) const fn new(config: ReactiveFade) -> Self {
        Self {
            config,
            cells: [[0; COLUMNS]; ROWS],
            last_color: 0,
            active: true,
            timer_ms: 0,
        }
    }

    pub(super) fn update(&mut self, config: ReactiveFade) {
        if config == self.config {
            return;
        }
        self.config = config;
        self.cells = [[0; COLUMNS]; ROWS];
        self.active = true;
    }

    pub(super) const fn needs_render(&self) -> bool {
        self.active
    }

    pub(super) fn press(&mut self, key: Key, now_ms: u32, variant: &Variant<ROWS, COLUMNS>) {
        let value = if matches!(self.config.coloring, ReactiveColoring::Palette) {
            pick_press_color(&mut self.last_color, now_ms) | PALETTE_PRESSED
        } else {
            PRESSED
        };
        let Some(cell) = self
            .cells
            .get_mut(key.row())
            .and_then(|row| row.get_mut(key.column()))
        else {
            return;
        };
        *cell = value;
        if key == variant.spacebar
            && let Some(row) = self.cells.get_mut(key.row())
        {
            for &column in variant.spacebar_fill {
                if let Some(cell) = row.get_mut(usize::from(column)) {
                    *cell = value;
                }
            }
        }
        self.active = true;
    }

    pub(super) fn render(&mut self, canvas: &mut impl Canvas, elapsed_ms: u32) {
        let palette = matches!(self.config.coloring, ReactiveColoring::Palette);
        let mut step_ms = u32::from(self.config.step_ms.clamp(1, STEP_MS_MAX));
        if palette {
            step_ms *= PALETTE_STEP_FACTOR;
        }
        let elapsed_ms = u16::try_from(elapsed_ms).unwrap_or(u16::MAX);
        let total_ms = u32::from(self.timer_ms.wrapping_add(elapsed_ms));
        let steps = (total_ms / step_ms).min(u32::from(u8::MAX));
        self.timer_ms = low_16(total_ms % step_ms);
        self.active = false;
        let config = self.config;
        for (row, cells) in self.cells.iter_mut().enumerate() {
            for (column, cell) in cells.iter_mut().enumerate() {
                let animating = match config.coloring {
                    ReactiveColoring::Palette => {
                        palette_cell(canvas, config, cell, steps, row, column)
                    }
                    ReactiveColoring::KeyColor => {
                        plain_cell(canvas, config, cell, steps, row, column)
                    }
                    ReactiveColoring::Fixed(color) => {
                        fixed_cell(canvas, config, opaque(color), cell, steps, row, column)
                    }
                };
                self.active |= animating;
            }
        }
    }
}

fn palette_cell(
    canvas: &mut impl Canvas,
    config: ReactiveFade,
    cell: &mut u8,
    steps: u32,
    row: usize,
    column: usize,
) -> bool {
    let value = *cell;
    let mut level = 0;
    let mut animating = false;
    if value & PALETTE_LEVEL_MASK != 0 {
        let count = u32::from(value >> PALETTE_LEVEL_SHIFT);
        if steps <= count {
            let count = if value & PALETTE_LEVEL_MASK == PALETTE_LEVEL_MASK {
                (count + 31) & 0x1F
            } else {
                (count - steps) & 0x1F
            };
            *cell = (value & PALETTE_COLOR_MASK) | (low_8(count) << PALETTE_LEVEL_SHIFT);
            level = low_8(255 * count / PALETTE_LEVEL_MAX);
        } else {
            *cell = value & PALETTE_COLOR_MASK;
        }
        animating = *cell & PALETTE_LEVEL_MASK != 0;
    }
    let level = mirrored_when_inverted(level, config.inverted);
    let color = palette_color(*cell & PALETTE_COLOR_MASK);
    set_key_scaled(
        canvas,
        row,
        column,
        color,
        mix_level(config.base_level, level),
    );
    animating
}

fn count_down(cell: &mut u8, steps: u32) -> u32 {
    let value = u32::from(*cell);
    let mut countdown = 0;
    if value != 0 {
        if steps <= value {
            countdown = if *cell == PRESSED {
                LEVEL_MAX
            } else {
                u32::from(low_8(value - steps))
            };
            *cell = low_8(countdown);
        } else {
            *cell = 0;
        }
    }
    countdown
}

fn plain_cell(
    canvas: &mut impl Canvas,
    config: ReactiveFade,
    cell: &mut u8,
    steps: u32,
    row: usize,
    column: usize,
) -> bool {
    let countdown = count_down(cell, steps);
    let level = mirrored_when_inverted(low_8(255 * countdown / LEVEL_MAX), config.inverted);
    canvas.scale_key(row, column, mix_level(config.base_level, level));
    countdown != 0
}

fn fixed_cell(
    canvas: &mut impl Canvas,
    config: ReactiveFade,
    color: Rgb,
    cell: &mut u8,
    steps: u32,
    row: usize,
    column: usize,
) -> bool {
    let countdown = count_down(cell, steps);
    let level = u32::from(mirrored_when_inverted(
        low_8(255 * countdown / LEVEL_MAX),
        config.inverted,
    ));
    if level == 0 {
        canvas.scale_key(row, column, config.base_level);
    } else if level < CROSSFADE_LEVEL {
        if let Some(led) = canvas.led_of(row, column)
            && let Some(current) = canvas.led(led)
        {
            let weight = i32::from(low_16(255 * (CROSSFADE_LEVEL - level) / 100));
            let target = canvas.stored(color);
            let mixed = rgb(
                crossfade(current.red, target.red, weight),
                crossfade(current.green, target.green, weight),
                crossfade(current.blue, target.blue, weight),
            );
            let mixed = if config.base_level == 255 {
                mixed
            } else {
                scale(mixed, config.base_level)
            };
            canvas.store_led(led, mixed);
        }
    } else {
        let dim = low_8(255 * (level - CROSSFADE_LEVEL) / (255 - CROSSFADE_LEVEL));
        set_key_scaled(
            canvas,
            row,
            column,
            color,
            mix_level(config.base_level, dim),
        );
    }
    countdown != 0
}

const fn mirrored_when_inverted(level: u8, inverted: bool) -> u8 {
    if inverted { !level } else { level }
}

fn crossfade(current: u8, target: u8, weight: i32) -> u8 {
    let mixed = (i32::from(current) - i32::from(target)) * weight + (i32::from(target) << 8);
    low_8(u32::from(low_16(mixed.cast_unsigned())) >> 8)
}

fn set_key_scaled(canvas: &mut impl Canvas, row: usize, column: usize, color: Rgb, factor: u8) {
    let color = if factor == 255 {
        color
    } else {
        scale(color, factor)
    };
    canvas.set_key(row, column, color);
}
