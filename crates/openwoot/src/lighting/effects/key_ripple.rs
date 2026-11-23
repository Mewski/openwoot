use openwoot_protocol::lighting::KeyRipple;

use super::{cells::Cells, press_color::pick_press_color};
use crate::{
    keys::Key,
    lighting::{
        canvas::Canvas,
        color::{palette_color, rgb, scale_channel},
        truncate::{low_8, low_16},
    },
};

const COLOR_MASK: u8 = 0x07;
const TAIL: u8 = 0x08;
const LEVEL_MASK: u8 = 0xF0;
const LEVEL_SHIFT: u8 = 4;
const LEVEL_MAX: u32 = 15;
const LEVEL_STEP: u8 = 1 << LEVEL_SHIFT;
const TAIL_FULL: u8 = 0xF8;
const STEPS_PER_SECOND_MAX: u8 = 50;

const NEW: u8 = 0;
const BURST: u8 = 1;
const UP_LEFT: u8 = 2;
const UP_RIGHT: u8 = 3;
const DOWN_LEFT: u8 = 4;
const DOWN_RIGHT: u8 = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct State<const ROWS: usize, const COLUMNS: usize> {
    config: KeyRipple,
    cells: Cells<ROWS, COLUMNS>,
    last_color: u8,
    active: bool,
    move_timer_ms: u16,
    decay_timer_ms: u16,
}

impl<const ROWS: usize, const COLUMNS: usize> State<ROWS, COLUMNS> {
    pub(super) const fn new(config: KeyRipple) -> Self {
        Self {
            config,
            cells: [[0; COLUMNS]; ROWS],
            last_color: 0,
            active: true,
            move_timer_ms: 0,
            decay_timer_ms: 0,
        }
    }

    pub(super) fn update(&mut self, config: KeyRipple) {
        if config == self.config {
            return;
        }
        self.config = config;
        self.cells = [[0; COLUMNS]; ROWS];
        self.active = true;
        self.move_timer_ms = 0;
        self.decay_timer_ms = 0;
    }

    pub(super) const fn needs_render(&self) -> bool {
        self.active
    }

    pub(super) fn press(&mut self, key: Key, now_ms: u32) {
        let color = if self.config.palette {
            pick_press_color(&mut self.last_color, now_ms)
        } else {
            1
        };
        if let Some(cell) = cell_mut(&mut self.cells, key.row(), key.column()) {
            *cell = color;
            self.active = true;
        }
    }

    pub(super) fn render(&mut self, canvas: &mut impl Canvas, elapsed_ms: u32) {
        let steps_per_second =
            u32::from(self.config.steps_per_second.clamp(1, STEPS_PER_SECOND_MAX));
        let decay_ms = u32::from(self.config.decay_step_ms.max(1));
        let moves = take_steps(&mut self.move_timer_ms, 1000 / steps_per_second, elapsed_ms);
        let decays = take_steps(&mut self.decay_timer_ms, decay_ms, elapsed_ms);
        for step in 0..moves {
            self.cells = advance(&self.cells, step + 1 == moves);
        }
        for _ in 0..decays {
            decay(&mut self.cells);
        }
        self.active = self.draw(canvas);
    }

    fn draw(&self, canvas: &mut impl Canvas) -> bool {
        let base = u32::from(self.config.base_level);
        let mut active = false;
        for (row, cells) in self.cells.iter().enumerate() {
            for (column, &cell) in cells.iter().enumerate() {
                if cell & COLOR_MASK == 0 {
                    if !self.config.inverted {
                        canvas.scale_key(row, column, self.config.base_level);
                    }
                    continue;
                }
                let fade = 255 * u32::from(cell >> LEVEL_SHIFT) / LEVEL_MAX;
                let level = match (self.config.inverted, cell & TAIL != 0) {
                    (true, true) => {
                        let signed = i64::from(fade) * (i64::from(base) - 255) - 256;
                        signed.to_le_bytes()[1]
                    }
                    (true, false) => self.config.base_level,
                    (false, true) => low_8((((base << 8) + fade * (255 - base)) & 0xFFFF) >> 8),
                    (false, false) => 255,
                };
                if self.config.palette {
                    let color = palette_color(cell & COLOR_MASK);
                    let color = if level == 255 {
                        color
                    } else {
                        rgb(
                            scale_channel(color.red, level),
                            scale_channel(color.green, level),
                            scale_channel(color.blue, level),
                        )
                    };
                    canvas.set_key(row, column, color);
                } else {
                    canvas.scale_key(row, column, level);
                }
                active = true;
            }
        }
        active
    }
}

fn take_steps(timer_ms: &mut u16, period_ms: u32, elapsed_ms: u32) -> u32 {
    let mut timer = u32::from(timer_ms.wrapping_add(low_16(elapsed_ms)));
    let mut steps = 0;
    while timer > period_ms {
        timer -= period_ms;
        steps += 1;
    }
    *timer_ms = low_16(timer);
    steps
}

const fn is_free(cell: u8) -> bool {
    cell & TAIL != 0 || cell & COLOR_MASK == 0
}

const fn head(cell: u8, direction: u8) -> u8 {
    (cell & 0x0F) | (direction << LEVEL_SHIFT)
}

fn cell_mut<const ROWS: usize, const COLUMNS: usize>(
    cells: &mut Cells<ROWS, COLUMNS>,
    row: usize,
    column: usize,
) -> Option<&mut u8> {
    cells.get_mut(row)?.get_mut(column)
}

fn enter<const ROWS: usize, const COLUMNS: usize>(
    next: &mut Cells<ROWS, COLUMNS>,
    row: Option<usize>,
    column: Option<usize>,
    cell: u8,
) {
    if let (Some(row), Some(column)) = (row, column)
        && let Some(target) = cell_mut(next, row, column)
        && is_free(*target)
    {
        *target = cell;
    }
}

fn put<const ROWS: usize, const COLUMNS: usize>(
    next: &mut Cells<ROWS, COLUMNS>,
    row: Option<usize>,
    column: Option<usize>,
    cell: u8,
) {
    if let (Some(row), Some(column)) = (row, column)
        && let Some(target) = cell_mut(next, row, column)
    {
        *target = cell;
    }
}

fn advance<const ROWS: usize, const COLUMNS: usize>(
    cells: &Cells<ROWS, COLUMNS>,
    last_step: bool,
) -> Cells<ROWS, COLUMNS> {
    let mut next = [[0; COLUMNS]; ROWS];
    for (row, line) in cells.iter().enumerate() {
        for (column, &cell) in line.iter().enumerate() {
            if cell & COLOR_MASK == 0 {
                continue;
            }
            let here = (Some(row), Some(column));
            if cell & TAIL != 0 {
                if cell & LEVEL_MASK != 0
                    && let Some(target) = cell_mut(&mut next, row, column)
                    && *target == 0
                {
                    *target = cell;
                }
                continue;
            }
            let up = row.checked_sub(1);
            let down = Some(row + 1).filter(|&below| below < ROWS);
            let left = column.checked_sub(1);
            let right = Some(column + 1).filter(|&next| next < COLUMNS);
            match cell >> LEVEL_SHIFT {
                NEW => {
                    let waiting = if last_step { head(cell, BURST) } else { cell };
                    put(&mut next, here.0, here.1, waiting);
                    continue;
                }
                BURST => {
                    put(&mut next, up, here.1, head(cell, UP_LEFT));
                    put(&mut next, here.0, left, head(cell, DOWN_LEFT));
                    put(&mut next, down, here.1, head(cell, DOWN_RIGHT));
                    put(&mut next, here.0, right, head(cell, UP_RIGHT));
                }
                UP_LEFT => {
                    enter(&mut next, up, here.1, cell);
                    enter(&mut next, here.0, left, cell);
                }
                UP_RIGHT => {
                    enter(&mut next, up, here.1, cell);
                    enter(&mut next, here.0, right, cell);
                }
                DOWN_LEFT => {
                    enter(&mut next, down, here.1, cell);
                    enter(&mut next, here.0, left, cell);
                }
                DOWN_RIGHT => {
                    enter(&mut next, down, here.1, cell);
                    enter(&mut next, here.0, right, cell);
                }
                _ => {}
            }
            put(&mut next, here.0, here.1, cell | TAIL_FULL);
        }
    }
    next
}

fn decay<const ROWS: usize, const COLUMNS: usize>(cells: &mut Cells<ROWS, COLUMNS>) {
    for cell in cells.iter_mut().flatten() {
        if *cell & COLOR_MASK != 0 && *cell & TAIL != 0 && *cell & LEVEL_MASK != 0 {
            *cell -= LEVEL_STEP;
        }
    }
}
