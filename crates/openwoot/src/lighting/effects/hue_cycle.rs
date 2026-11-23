use openwoot_protocol::lighting::{HueCycle, HueRange};

use super::hue::{Hues, Phase, hue_to_rgb};
use crate::lighting::canvas::Canvas;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State {
    config: HueCycle,
    hues: Hues,
    phase: Phase,
}

impl State {
    pub(super) const fn new(config: HueCycle) -> Self {
        Self {
            config,
            hues: Hues::of(config.hues),
            phase: Phase::new(),
        }
    }

    pub(super) fn update(&mut self, config: HueCycle) {
        if config != self.config {
            *self = Self::new(config);
        }
    }

    pub(super) fn render<const ROWS: usize, const COLUMNS: usize>(
        &mut self,
        canvas: &mut impl Canvas,
        elapsed_ms: u32,
    ) {
        self.phase.advance(self.config.period, elapsed_ms, true);
        let position = i32::from(self.phase.turn() >> 8);
        let first = i32::from(self.hues.first);
        let last = i32::from(self.hues.last);
        let hue = if self.config.hues == HueRange::FullCircle {
            ((first << 8) + (last - first) * position) >> 8
        } else if position & 0x80 == 0 {
            (((first << 8) + 2 * position * (last - first) + 0x80) & 0xFFFF) >> 8
        } else {
            (((last << 8) + (first - last) * 2 * (position - 0x80) + 0x80) & 0xFFFF) >> 8
        };
        let color = hue_to_rgb(hue.to_le_bytes()[0]);
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                canvas.set_key(row, column, color);
            }
        }
    }
}
