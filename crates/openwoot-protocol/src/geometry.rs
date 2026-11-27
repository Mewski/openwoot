use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    configuration::MAX_KEYS,
    wire::{Decode, Encode},
};

pub const UNIT: u16 = 100;
pub const ISO_ENTER_STEP: u16 = UNIT / 4;

pub type Keys = Vec<Option<Key>, MAX_KEYS>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode)]
pub struct Key {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub shape: Shape,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema, Encode, Decode,
)]
pub enum Shape {
    #[default]
    Rectangle,
    IsoEnter,
}

impl Key {
    #[must_use]
    pub const fn contains(&self, x: u16, y: u16) -> bool {
        let (Some(right), Some(down)) = (x.checked_sub(self.x), y.checked_sub(self.y)) else {
            return false;
        };
        let inside = right < self.width && down < self.height;
        match self.shape {
            Shape::Rectangle => inside,
            Shape::IsoEnter => inside && (down < UNIT || right >= ISO_ENTER_STEP),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISO_ENTER: Key = Key {
        x: 1350,
        y: 225,
        width: 150,
        height: 200,
        shape: Shape::IsoEnter,
    };

    #[test]
    fn an_iso_enter_spans_its_box_but_the_lower_left_step() {
        assert!(ISO_ENTER.contains(1350, 225));
        assert!(ISO_ENTER.contains(1374, 324));
        assert!(!ISO_ENTER.contains(1374, 325));
        assert!(!ISO_ENTER.contains(1350, 424));
        assert!(ISO_ENTER.contains(1375, 325));
        assert!(ISO_ENTER.contains(1499, 424));
        assert!(!ISO_ENTER.contains(1500, 300));
        assert!(!ISO_ENTER.contains(1400, 425));
        assert!(!ISO_ENTER.contains(1349, 300));
        assert!(!ISO_ENTER.contains(1400, 224));
    }

    #[test]
    fn a_rectangle_spans_its_whole_box() {
        let key = Key {
            shape: Shape::Rectangle,
            ..ISO_ENTER
        };
        assert!(key.contains(1350, 424));
        assert!(key.contains(1499, 225));
        assert!(!key.contains(1500, 225));
        assert!(!key.contains(1350, 425));
    }
}
