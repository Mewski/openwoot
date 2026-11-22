use super::{analog_levels::Levels, cells::Cells};
use crate::lighting::{canvas::Canvas, color::mix_level};

pub(super) fn render<const ROWS: usize, const COLUMNS: usize>(
    levels: &mut Levels<ROWS, COLUMNS>,
    canvas: &mut impl Canvas,
) {
    let mut mask: Cells<ROWS, COLUMNS> = [[0; COLUMNS]; ROWS];
    for (row, cells) in levels.cells().iter().enumerate() {
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
    let base_level = levels.base_level();
    for (row, cells) in mask.iter().enumerate() {
        for (column, &level) in cells.iter().enumerate() {
            let factor = if level == 0 {
                base_level
            } else {
                mix_level(base_level, level)
            };
            canvas.scale_key(row, column, factor);
        }
    }
    levels.rendered();
}
