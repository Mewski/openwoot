use libm::{atan2f, floorf, fmaf, sqrtf};

use super::Cells;
use crate::lighting::{
    color::{blend, opaque},
    config::PolarGradient,
    frame::Canvas,
};

const PHASE_MODULUS: i64 = 65_535;
const TWO_PI: f32 = 6.283_185_5;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct State<const ROWS: usize, const COLUMNS: usize> {
    config: PolarGradient,
    phase: u16,
    distances: Cells<ROWS, COLUMNS>,
    angles: Cells<ROWS, COLUMNS>,
}

impl<const ROWS: usize, const COLUMNS: usize> State<ROWS, COLUMNS> {
    pub(crate) fn new(config: PolarGradient) -> Self {
        let mut distances = [[0; COLUMNS]; ROWS];
        let mut angles = [[0; COLUMNS]; ROWS];
        let center_row = count(ROWS) / 2.0;
        let center_column = count(COLUMNS) / 2.0;
        for (row, (distances, angles)) in distances.iter_mut().zip(&mut angles).enumerate() {
            let dy = center_row - count(row);
            for (column, (distance, angle)) in distances.iter_mut().zip(angles).enumerate() {
                let dx = center_column - count(column);
                *distance = to_byte(sqrtf(dy * dy + dx * dx) * 16.0);
                *angle = to_byte((atan2f(dy, dx) / TWO_PI + 0.5) * 255.0);
            }
        }
        Self {
            config,
            phase: 0,
            distances,
            angles,
        }
    }

    pub(crate) const fn update(&mut self, config: PolarGradient) {
        self.config = config;
        self.phase = 0;
    }

    pub(crate) fn render(&mut self, canvas: &mut impl Canvas, elapsed_ms: u32) {
        let advance = i64::from(elapsed_ms) * i64::from(self.config.speed);
        let phase = (i64::from(self.phase) + advance).rem_euclid(PHASE_MODULUS);
        self.phase = u16::try_from(phase).unwrap_or_default();
        let from = opaque(self.config.from);
        let to = opaque(self.config.to);
        let phase = f32::from(self.phase) / 65_535.0;
        let twist = f32::from(self.config.twist) / 127.0 * 0.1;
        for (row, (distances, angles)) in self.distances.iter().zip(&self.angles).enumerate() {
            for (column, (&distance, &angle)) in distances.iter().zip(angles).enumerate() {
                let angle = f32::from(angle) / 255.0 - 0.5 + phase;
                let distance = f32::from(distance) * 0.0625;
                let turns = fmaf(twist, distance, angle);
                let position = to_byte(fmaf(turns - floorf(turns), 255.0, 0.5));
                let color = match position {
                    0 | 255 => from,
                    128 => to,
                    1..128 => blend(from, to, 2 * position),
                    _ => blend(from, to, !(2 * (position - 128))),
                };
                canvas.set_key(row, column, color);
            }
        }
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "matrix dimensions are far below the float mantissa"
)]
fn count(value: usize) -> f32 {
    value as f32
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is converted to an integer and its low byte kept, as the effect defines"
)]
fn to_byte(value: f32) -> u8 {
    (value as u32).to_le_bytes()[0]
}
