use super::travel::travel_level_12;
use crate::lighting::{
    canvas::Canvas,
    config::LevelMeter,
    input::{KeyTravel, TRAVEL_FULL},
    leds::Key,
    truncate::low_8,
};

const ROW: usize = 1;
const FIRST_COLUMN: usize = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct State {
    config: LevelMeter,
    level: u16,
    key: Option<Key>,
    dirty: bool,
}

impl State {
    pub(crate) const fn new(config: LevelMeter) -> Self {
        Self {
            config,
            level: 0,
            key: None,
            dirty: true,
        }
    }

    pub(crate) const fn update(&mut self, config: LevelMeter) {
        *self = Self::new(config);
    }

    pub(crate) const fn needs_render(&self) -> bool {
        self.dirty
    }

    pub(crate) const fn release_all(&mut self) {
        self.level = 0;
        self.dirty = true;
    }

    pub(crate) fn feed(&mut self, key: Key, travel: u16, input: &impl KeyTravel) {
        let level = travel_level_12(self.config.gamepad_curve, travel, input);
        if self.level < level {
            self.key = Some(key);
            self.level = level;
            self.dirty = true;
        } else if self.key == Some(key) && self.level != level {
            self.level = level;
            self.dirty = true;
        }
    }

    pub(crate) fn render(&mut self, canvas: &mut impl Canvas, segments: u8) {
        let segments = u32::from(segments);
        let base = u32::from(self.config.base_level);
        let level = u32::from(self.level);
        for segment in 0..segments {
            let lower = segment * u32::from(TRAVEL_FULL) / segments;
            let upper = (segment + 1) * u32::from(TRAVEL_FULL) / segments;
            let column = FIRST_COLUMN + usize::try_from(segment).unwrap_or(usize::MAX);
            if level < lower {
                canvas.scale_key(ROW, column, self.config.base_level);
            } else if level < upper {
                let fraction = 255 * (level - lower) / (upper - lower);
                canvas.scale_key(
                    ROW,
                    column,
                    low_8(((base << 8) + fraction * (255 - base)) >> 8),
                );
            }
        }
        self.dirty = false;
    }
}
