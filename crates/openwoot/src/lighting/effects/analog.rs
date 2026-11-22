use super::{Cells, low_8, travel_level_8};
use crate::lighting::{
    color::{mix_level, rgb},
    config::{AnalogLevel, AnalogRgbFill},
    frame::Canvas,
    input::KeyTravel,
    leds::Key,
};

const SPOTLIGHT_LEVELS_PER_KEY: u8 = 30;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Levels<const ROWS: usize, const COLUMNS: usize> {
    config: AnalogLevel,
    cells: Cells<ROWS, COLUMNS>,
    dirty: bool,
}

impl<const ROWS: usize, const COLUMNS: usize> Levels<ROWS, COLUMNS> {
    pub(crate) const fn new(config: AnalogLevel) -> Self {
        Self {
            config,
            cells: [[0; COLUMNS]; ROWS],
            dirty: true,
        }
    }

    pub(crate) const fn update(&mut self, config: AnalogLevel) {
        *self = Self::new(config);
    }

    pub(crate) const fn needs_render(&self) -> bool {
        self.dirty
    }

    pub(crate) fn feed(&mut self, key: Key, travel: u16, input: &impl KeyTravel) {
        let level = travel_level_8(self.config.gamepad_curve, travel, input);
        if let Some(cell) = self
            .cells
            .get_mut(key.row())
            .and_then(|row| row.get_mut(key.column()))
            && *cell != level
        {
            *cell = level;
            self.dirty = true;
        }
    }

    pub(crate) fn release_all(&mut self, always_dirty: bool) {
        let pressed = self.cells.iter().flatten().any(|&level| level != 0);
        self.cells = [[0; COLUMNS]; ROWS];
        if pressed || always_dirty {
            self.dirty = true;
        }
    }

    pub(crate) fn render_spotlight(&mut self, canvas: &mut impl Canvas) {
        let mut mask: Cells<ROWS, COLUMNS> = [[0; COLUMNS]; ROWS];
        for (row, cells) in self.cells.iter().enumerate() {
            for (column, &level) in cells.iter().enumerate() {
                let reach = usize::from(level / SPOTLIGHT_LEVELS_PER_KEY);
                for row_offset in 0..reach {
                    for column_offset in 0..reach {
                        for (row, column) in [
                            (
                                row.checked_add(row_offset),
                                column.checked_add(column_offset),
                            ),
                            (
                                row.checked_sub(row_offset),
                                column.checked_sub(column_offset),
                            ),
                            (
                                row.checked_add(row_offset),
                                column.checked_sub(column_offset),
                            ),
                            (
                                row.checked_sub(row_offset),
                                column.checked_add(column_offset),
                            ),
                        ] {
                            if let (Some(row), Some(column)) = (row, column)
                                && let Some(cell) =
                                    mask.get_mut(row).and_then(|cells| cells.get_mut(column))
                            {
                                *cell = level;
                            }
                        }
                    }
                }
            }
        }
        for (row, cells) in mask.iter().enumerate() {
            for (column, &lit) in cells.iter().enumerate() {
                let factor = if lit == 0 {
                    self.config.base_level
                } else {
                    255
                };
                canvas.scale_key(row, column, factor);
            }
        }
        self.dirty = false;
    }

    pub(crate) fn render_glow(&mut self, canvas: &mut impl Canvas) {
        let mut mask: Cells<ROWS, COLUMNS> = [[0; COLUMNS]; ROWS];
        for (row, cells) in self.cells.iter().enumerate() {
            for (column, &level) in cells.iter().enumerate() {
                if level == 0 {
                    continue;
                }
                for near_row in row.saturating_sub(1)..=row + 1 {
                    for near_column in column.saturating_sub(1)..=column + 1 {
                        if let Some(cell) = mask
                            .get_mut(near_row)
                            .and_then(|cells| cells.get_mut(near_column))
                            && *cell < level
                        {
                            *cell = level;
                        }
                    }
                }
            }
        }
        for (row, cells) in mask.iter().enumerate() {
            for (column, &level) in cells.iter().enumerate() {
                let factor = if level == 0 {
                    self.config.base_level
                } else {
                    mix_level(self.config.base_level, level)
                };
                canvas.scale_key(row, column, factor);
            }
        }
        self.dirty = false;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fill {
    config: AnalogRgbFill,
    red: u8,
    green: u8,
    blue: u8,
}

impl Fill {
    pub(crate) const fn new(config: AnalogRgbFill) -> Self {
        Self {
            config,
            red: 0,
            green: 0,
            blue: 0,
        }
    }

    pub(crate) const fn update(&mut self, config: AnalogRgbFill) {
        *self = Self::new(config);
    }

    pub(crate) const fn release_all(&mut self) {
        self.red = 0;
        self.green = 0;
        self.blue = 0;
    }

    pub(crate) fn feed<const ROWS: usize, const COLUMNS: usize>(
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

    pub(crate) fn render<const ROWS: usize, const COLUMNS: usize>(&self, canvas: &mut impl Canvas) {
        let color = rgb(self.red, self.green, self.blue);
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                canvas.set_key(row, column, color);
            }
        }
    }
}

pub(crate) fn linear_level(travel: u16) -> u8 {
    low_8(255 * u32::from(travel) / u32::from(super::TRAVEL_FULL))
}
