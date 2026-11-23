use openwoot_protocol::lighting::{HueRange, RainbowWave};

use super::hue::{Hues, Phase, hue_to_rgb};
use crate::lighting::canvas::Canvas;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State {
    config: RainbowWave,
    hues: Hues,
    phase: Phase,
}

impl State {
    pub(super) const fn new(config: RainbowWave) -> Self {
        Self {
            config,
            hues: Hues::of(config.hues),
            phase: Phase::new(),
        }
    }

    pub(super) fn update(&mut self, config: RainbowWave) {
        if config.hues != self.config.hues {
            self.hues = Hues::of(config.hues);
        }
        self.config = config;
    }

    pub(super) fn render<const ROWS: usize, const COLUMNS: usize>(
        &mut self,
        canvas: &mut impl Canvas,
        elapsed_ms: u32,
    ) {
        self.phase
            .advance(self.config.period, elapsed_ms, self.config.forward);
        let (first, second) = if self.config.forward {
            (i32::from(self.hues.first), i32::from(self.hues.last))
        } else {
            (i32::from(self.hues.last), i32::from(self.hues.first))
        };
        let offset = usize::from(self.phase.turn() >> 8);
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
