use super::cells::Cells;
use crate::lighting::{canvas::Canvas, color::rgb, config::KeyTest, input::KeyEvent, leds::Key};

const IDLE: u8 = 0;
const ACTUATED: u8 = 1;
const PRESSED: u8 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct State<const ROWS: usize, const COLUMNS: usize> {
    config: KeyTest,
    marks: Cells<ROWS, COLUMNS>,
    dirty: bool,
}

impl<const ROWS: usize, const COLUMNS: usize> State<ROWS, COLUMNS> {
    pub(crate) const fn new(config: KeyTest) -> Self {
        Self {
            config,
            marks: [[IDLE; COLUMNS]; ROWS],
            dirty: true,
        }
    }

    pub(crate) const fn update(&mut self, config: KeyTest) {
        *self = Self::new(config);
    }

    pub(crate) const fn needs_render(&self) -> bool {
        self.dirty
    }

    pub(crate) fn event(&mut self, key: Key, event: KeyEvent) {
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

    pub(crate) fn render(&mut self, canvas: &mut impl Canvas) {
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
