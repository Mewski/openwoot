use openwoot_protocol::lighting::{Scanner, ScannerDirection};

use crate::lighting::{canvas::Canvas, truncate::low_16};

const STEPS_PER_SECOND_MAX: u8 = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State {
    config: Scanner,
    window_end: i16,
    window_start: i16,
    direction: ScannerDirection,
    timer_ms: u16,
}

impl State {
    pub(super) fn new<const ROWS: usize, const COLUMNS: usize>(config: Scanner) -> Self {
        Self {
            config,
            window_end: window_width::<ROWS, COLUMNS>(config.width, config.direction),
            window_start: 0,
            direction: config.direction,
            timer_ms: 0,
        }
    }

    pub(super) fn update<const ROWS: usize, const COLUMNS: usize>(&mut self, config: Scanner) {
        let direction_changed = config.direction != self.config.direction;
        if direction_changed {
            self.direction = config.direction;
        }
        if direction_changed || config.width != self.config.width {
            self.window_end = window_width::<ROWS, COLUMNS>(config.width, self.direction);
            self.window_start = 0;
        }
        self.config = config;
    }

    pub(super) fn render<const ROWS: usize, const COLUMNS: usize>(
        &mut self,
        canvas: &mut impl Canvas,
        first_key_row: usize,
        elapsed_ms: u32,
    ) {
        let mut timer_ms = self.timer_ms.wrapping_add(low_16(elapsed_ms));
        let step_ms = 1000 / u16::from(self.config.steps_per_second.clamp(1, STEPS_PER_SECOND_MAX));
        let width = i32::from(self.window_end) - i32::from(self.window_start);
        while step_ms < timer_ms {
            timer_ms -= step_ms;
            let forward = matches!(
                self.direction,
                ScannerDirection::Right | ScannerDirection::Down
            );
            let shift = if forward { 1 } else { -1 };
            self.window_end = self.window_end.saturating_add(shift);
            self.window_start = self.window_start.saturating_add(shift);
            let end = i32::from(self.window_end);
            self.direction = match self.direction {
                ScannerDirection::Left if end <= (width + 1) >> 1 => ScannerDirection::Right,
                ScannerDirection::Right if (width >> 1) + last(COLUMNS) < end => {
                    ScannerDirection::Left
                }
                ScannerDirection::Up if end <= ((width + 1) >> 1) + index(first_key_row) => {
                    ScannerDirection::Down
                }
                ScannerDirection::Down if (width >> 1) + last(ROWS) < end => ScannerDirection::Up,
                unchanged => unchanged,
            };
        }
        self.timer_ms = timer_ms;

        let start = i32::from(self.window_start);
        let end = i32::from(self.window_end);
        let outside = |position: usize| {
            let position = index(position);
            position < start || position >= end
        };
        let level = self.config.outside_level;
        if moves_columns(self.direction) {
            for column in (0..COLUMNS).filter(|column| outside(*column)) {
                for row in 0..ROWS {
                    canvas.scale_key(row, column, level);
                }
            }
        } else {
            for row in (first_key_row..ROWS).filter(|row| outside(*row)) {
                for column in 0..COLUMNS {
                    canvas.scale_key(row, column, level);
                }
            }
        }
    }
}

fn window_width<const ROWS: usize, const COLUMNS: usize>(
    width: u8,
    direction: ScannerDirection,
) -> i16 {
    let limit = if moves_columns(direction) {
        COLUMNS
    } else {
        ROWS
    };
    i16::try_from(usize::from(width).min(limit)).unwrap_or(i16::MAX)
}

const fn moves_columns(direction: ScannerDirection) -> bool {
    matches!(direction, ScannerDirection::Left | ScannerDirection::Right)
}

fn index(position: usize) -> i32 {
    i32::try_from(position).unwrap_or(i32::MAX)
}

fn last(count: usize) -> i32 {
    index(count) - 1
}
