use super::travel::travel_level_8;
use crate::{
    keys::Key,
    lighting::{canvas::Canvas, color::rgb, config::AnalogRgbFill, input::KeyTravel},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State {
    config: AnalogRgbFill,
    red: u8,
    green: u8,
    blue: u8,
}

impl State {
    pub(super) const fn new(config: AnalogRgbFill) -> Self {
        Self {
            config,
            red: 0,
            green: 0,
            blue: 0,
        }
    }

    pub(super) const fn update(&mut self, config: AnalogRgbFill) {
        *self = Self::new(config);
    }

    pub(super) const fn release_all(&mut self) {
        self.red = 0;
        self.green = 0;
        self.blue = 0;
    }

    pub(super) fn feed<const ROWS: usize, const COLUMNS: usize>(
        &mut self,
        key: Key,
        travel: u16,
        input: &impl KeyTravel,
    ) {
        if key.row() + 1 != ROWS {
            return;
        }
        let level = travel_level_8(self.config.gamepad_curve, travel, input);
        match COLUMNS.checked_sub(key.column()) {
            Some(3) => self.red = level,
            Some(2) => self.green = level,
            Some(1) => self.blue = level,
            _ => {}
        }
    }

    pub(super) fn render<const ROWS: usize, const COLUMNS: usize>(&self, canvas: &mut impl Canvas) {
        let color = rgb(self.red, self.green, self.blue);
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                canvas.set_key(row, column, color);
            }
        }
    }
}
