use std::collections::BTreeSet;

use openwoot_protocol::{
    geometry::{self, UNIT},
    lighting::{KeyColors, Rgb},
};

use crate::store::Keyboard;

pub(super) fn uniform(count: usize, color: Rgb) -> KeyColors {
    let mut colors = KeyColors::new();
    for _ in 0..count {
        if colors.push(color).is_err() {
            break;
        }
    }
    colors
}

pub(super) fn reach(keyboard: &Keyboard, keys: impl IntoIterator<Item = usize>) -> BTreeSet<usize> {
    let mut reached = BTreeSet::new();
    for key in keys {
        reached.insert(key);
        if let Some(geometry) = &keyboard.geometry {
            reached.extend(companions(
                geometry,
                keyboard.columns(),
                keyboard.key_count(),
                key,
            ));
        }
    }
    reached
}

pub(super) fn paint(colors: &mut KeyColors, keys: &BTreeSet<usize>, color: Rgb) {
    for key in keys {
        if let Some(slot) = colors.get_mut(*key) {
            *slot = color;
        }
    }
}

fn companions(
    geometry: &[Option<geometry::Key>],
    columns: usize,
    count: usize,
    key: usize,
) -> Vec<usize> {
    let (Some(Some(shape)), Some(row)) = (geometry.get(key), key.checked_div(columns)) else {
        return Vec::new();
    };
    let first = row * columns;
    (first..(first + columns).min(count))
        .filter(|index| *index != key && geometry.get(*index).is_none_or(Option::is_none))
        .filter(|index| {
            index
                .checked_rem(columns)
                .is_some_and(|column| covers(shape, column))
        })
        .collect()
}

fn covers(shape: &geometry::Key, column: usize) -> bool {
    let Ok(column) = u32::try_from(column) else {
        return false;
    };
    let unit = u32::from(UNIT);
    let centre = column * unit + unit / 2;
    let start = u32::from(shape.x);
    (start..start + u32::from(shape.width)).contains(&centre)
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::geometry::Shape;

    use super::*;

    const COLUMNS: usize = 17;
    const COUNT: usize = 6 * COLUMNS;
    const BOTTOM: usize = 5 * COLUMNS;
    const NONE: [usize; 0] = [];

    fn key(x: u16, width: u16) -> geometry::Key {
        geometry::Key {
            x,
            y: 525,
            width,
            height: 100,
            shape: Shape::Rectangle,
        }
    }

    fn bottom_row() -> Vec<Option<geometry::Key>> {
        let mut geometry = vec![None; COUNT];
        for (column, shape) in [
            (0, key(0, 125)),
            (1, key(125, 125)),
            (2, key(250, 125)),
            (6, key(375, 625)),
            (10, key(1000, 125)),
            (11, key(1125, 125)),
            (12, key(1250, 125)),
            (14, key(1525, 100)),
            (15, key(1625, 100)),
            (16, key(1725, 100)),
        ] {
            geometry[BOTTOM + column] = Some(shape);
        }
        geometry
    }

    #[test]
    fn the_space_bar_carries_the_leds_under_its_span() {
        let geometry = bottom_row();
        let space = companions(&geometry, COLUMNS, COUNT, BOTTOM + 6);
        let columns: Vec<usize> = space.iter().map(|index| index - BOTTOM).collect();
        assert_eq!(columns, [4, 5, 7, 8, 9]);
    }

    #[test]
    fn a_key_takes_only_the_positions_its_span_covers() {
        let geometry = bottom_row();
        assert_eq!(
            companions(&geometry, COLUMNS, COUNT, BOTTOM + 2),
            [BOTTOM + 3]
        );
        assert_eq!(
            companions(&geometry, COLUMNS, COUNT, BOTTOM + 12),
            [BOTTOM + 13]
        );
        assert_eq!(companions(&geometry, COLUMNS, COUNT, BOTTOM), NONE);
        assert_eq!(companions(&geometry, COLUMNS, COUNT, BOTTOM + 14), NONE);
    }

    #[test]
    fn positions_without_a_key_have_no_companions() {
        let geometry = bottom_row();
        assert_eq!(companions(&geometry, COLUMNS, COUNT, BOTTOM + 4), NONE);
        assert_eq!(companions(&geometry, COLUMNS, COUNT, 0), NONE);
        assert_eq!(companions(&geometry, 0, COUNT, BOTTOM + 6), NONE);
    }

    #[test]
    fn positions_past_the_geometry_count_as_positions_without_a_key() {
        let mut geometry = vec![None; COLUMNS];
        geometry[0] = Some(geometry::Key {
            x: 0,
            y: 0,
            width: 300,
            height: 100,
            shape: Shape::Rectangle,
        });
        geometry.truncate(1);
        assert_eq!(companions(&geometry, COLUMNS, COUNT, 0), [1, 2]);
    }

    #[test]
    fn painting_changes_only_the_reached_keys() {
        let black = Rgb::default();
        let red = Rgb {
            red: 255,
            green: 0,
            blue: 0,
        };
        let mut colors = uniform(4, black);
        paint(&mut colors, &BTreeSet::from([1, 3, 9]), red);
        assert_eq!(colors.as_slice(), [black, red, black, red]);
    }
}
