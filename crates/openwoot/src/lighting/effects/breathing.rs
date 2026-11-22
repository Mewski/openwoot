use crate::lighting::{
    canvas::Canvas,
    config::Breathing,
    truncate::{low_8, low_16},
};

const STEP_MS_MAX: u8 = 40;
const LEVEL_MAX: u8 = 250;
const LEVEL_STEP: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State {
    config: Breathing,
    level: u8,
    falling: bool,
    timer_ms: u16,
}

impl State {
    pub(super) const fn new(config: Breathing) -> Self {
        Self {
            config,
            level: 0,
            falling: false,
            timer_ms: 0,
        }
    }

    pub(super) const fn update(&mut self, config: Breathing) {
        self.config = config;
    }

    pub(super) fn render<const ROWS: usize, const COLUMNS: usize>(
        &mut self,
        canvas: &mut impl Canvas,
        elapsed_ms: u32,
    ) {
        let step_ms = u16::from(self.config.step_ms.clamp(1, STEP_MS_MAX));
        let mut timer_ms = self.timer_ms.wrapping_add(low_16(elapsed_ms));
        while timer_ms > step_ms {
            timer_ms -= step_ms;
            if self.falling {
                if self.level == 0 {
                    self.falling = false;
                } else {
                    self.level -= LEVEL_STEP;
                }
            } else if self.level < LEVEL_MAX {
                self.level += LEVEL_STEP;
            } else {
                self.falling = true;
            }
        }
        self.timer_ms = timer_ms;

        let min = u32::from(self.config.min_level);
        let level = 255 * u32::from(self.level) / u32::from(LEVEL_MAX);
        let scale = low_8(((min << 8) + level * (255 - min)) >> 8);
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                canvas.scale_key(row, column, scale);
            }
        }
    }
}
