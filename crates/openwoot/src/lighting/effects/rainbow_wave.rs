use super::hue::{Hues, hue_to_rgb, phase_step};
use crate::lighting::{
    canvas::Canvas,
    config::{HueRange, RainbowWave},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct State {
    config: RainbowWave,
    hues: Hues,
    phase: u16,
}

impl State {
    pub(crate) const fn new(config: RainbowWave) -> Self {
        Self {
            config,
            hues: Hues::of(config.hues),
            phase: 0,
        }
    }

    pub(crate) fn update(&mut self, config: RainbowWave) {
        if config.hues != self.config.hues {
            self.hues = Hues::of(config.hues);
        }
        self.config = config;
    }

    pub(crate) fn render<const ROWS: usize, const COLUMNS: usize>(
        &mut self,
        canvas: &mut impl Canvas,
        elapsed_ms: u32,
    ) {
        let step = phase_step(self.config.period, elapsed_ms);
        let (first, second) = if self.config.forward {
            self.phase = self.phase.wrapping_add(step);
            (i32::from(self.hues.first), i32::from(self.hues.last))
        } else {
            self.phase = self.phase.wrapping_sub(step);
            (i32::from(self.hues.last), i32::from(self.hues.first))
        };
        let offset = usize::from(self.phase >> 8);
        for column in 0..COLUMNS {
            let hue = if self.config.hues == HueRange::FullCircle {
                let position = wrap(offset + 255 * column / COLUMNS);
                ((first << 8) + position * (second - first)) >> 8
            } else {
                let position = wrap(offset + 255 * column / (2 * COLUMNS));
                if position < 128 {
                    ((first << 8) + 2 * position * (second - first) + 128) >> 8
                } else {
                    ((second << 8) + 2 * (position - 128) * (first - second) + 128) >> 8
                }
            };
            let color = hue_to_rgb(hue.to_le_bytes()[0]);
            for row in 0..ROWS {
                canvas.set_key(row, column, color);
            }
        }
    }
}

fn wrap(position: usize) -> i32 {
    i32::from(position.to_le_bytes()[0])
}
