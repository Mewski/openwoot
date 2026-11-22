pub const LAYERS: usize = 4;

const TOP_LAYER: u8 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Layers {
    active: u8,
    locked: u8,
}

impl Layers {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: 0,
            locked: 0,
        }
    }

    #[must_use]
    pub const fn active(&self) -> u8 {
        self.active
    }

    #[must_use]
    pub const fn locked(&self) -> u8 {
        self.locked
    }

    pub const fn momentary(&mut self, layer: u8, pressed: bool) -> u8 {
        if pressed {
            self.active = if self.locked == layer { 0 } else { layer };
        } else if self.active == layer || self.locked == layer {
            self.active = self.locked;
        }
        self.active
    }

    pub const fn lock(&mut self, layer: u8) -> Option<u8> {
        if layer > TOP_LAYER {
            return None;
        }
        let layer = if self.locked == layer { 0 } else { layer };
        self.locked = layer;
        self.active = layer;
        Some(layer)
    }

    pub const fn reset(&mut self) {
        *self = Self::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_momentary_layer_returns_to_the_locked_layer() {
        let mut layers = Layers::new();
        assert_eq!(layers.momentary(1, true), 1);
        assert_eq!(layers.momentary(1, false), 0);
        assert_eq!(layers.lock(2), Some(2));
        assert_eq!(layers.momentary(1, true), 1);
        assert_eq!(layers.momentary(1, false), 2);
        assert_eq!(layers.momentary(2, true), 0);
        assert_eq!(layers.momentary(2, false), 2);
        assert_eq!(layers.lock(2), Some(0));
        assert_eq!(layers.locked(), 0);
        assert_eq!(layers.lock(4), None);
    }
}
