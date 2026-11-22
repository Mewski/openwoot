use super::{cells::Cells, travel::travel_level_8};
use crate::lighting::{config::AnalogLevel, input::KeyTravel, leds::Key};

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

    pub(super) const fn cells(&self) -> &Cells<ROWS, COLUMNS> {
        &self.cells
    }

    pub(super) const fn base_level(&self) -> u8 {
        self.config.base_level
    }

    pub(super) const fn rendered(&mut self) {
        self.dirty = false;
    }
}
