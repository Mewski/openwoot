use openwoot_protocol::lighting::KeyTest;

use super::cells::Cells;
use crate::{
    keys::Key,
    lighting::{canvas::Canvas, color::rgb, input::KeyEvent},
};

const IDLE: u8 = 0;
const ACTUATED: u8 = 1;
const PRESSED: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State<const ROWS: usize, const COLUMNS: usize> {
    config: KeyTest,
    marks: Cells<ROWS, COLUMNS>,
    dirty: bool,
}

impl<const ROWS: usize, const COLUMNS: usize> State<ROWS, COLUMNS> {
    pub(super) const fn new(config: KeyTest) -> Self {
        Self {
            config,
            marks: [[IDLE; COLUMNS]; ROWS],
            dirty: true,
        }
    }

    pub(super) const fn update(&mut self, config: KeyTest) {
        *self = Self::new(config);
    }

    pub(super) const fn needs_render(&self) -> bool {
        self.dirty
    }

    pub(super) fn event(&mut self, key: Key, event: KeyEvent) {
        let Some(mark) = self
            .marks
            .get_mut(key.row())
            .and_then(|row| row.get_mut(key.column()))
        else {
            return;
        };
        *mark = match event {
            KeyEvent::Actuated => ACTUATED,
            KeyEvent::Reset => IDLE,
            KeyEvent::Released if *mark == PRESSED => ACTUATED,
            KeyEvent::Released => *mark,
            KeyEvent::Pressed => PRESSED,
        };
        self.dirty = true;
    }

    pub(super) fn render(&mut self, canvas: &mut impl Canvas) {
        for (row, marks) in self.marks.iter().enumerate() {
            for (column, &mark) in marks.iter().enumerate() {
                match mark {
                    ACTUATED => canvas.set_key(row, column, rgb(255, 0, 0)),
                    PRESSED => canvas.set_key(row, column, rgb(0, 255, 0)),
                    _ => canvas.scale_key(row, column, self.config.base_level),
                }
            }
        }
        self.dirty = false;
    }
}
