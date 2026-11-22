use super::{analog_levels::Levels, cells::Cells};
use crate::lighting::canvas::Canvas;

const SPOTLIGHT_LEVELS_PER_KEY: u8 = 30;

pub(super) fn render<const ROWS: usize, const COLUMNS: usize>(
    levels: &mut Levels<ROWS, COLUMNS>,
    canvas: &mut impl Canvas,
) {
    let mut mask: Cells<ROWS, COLUMNS> = [[0; COLUMNS]; ROWS];
    for (row, cells) in levels.cells().iter().enumerate() {
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
    let base_level = levels.base_level();
    for (row, cells) in mask.iter().enumerate() {
        for (column, &lit) in cells.iter().enumerate() {
            let factor = if lit == 0 { base_level } else { 255 };
            canvas.scale_key(row, column, factor);
        }
    }
    levels.rendered();
}
