pub const LAYERS: usize = 4;

const TOP_LAYER: u8 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Layers {
    active: u8,
    locked: u8,
    held: u8,
}

impl Layers {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: 0,
            locked: 0,
            held: 0,
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
        if layer == 0 || layer > TOP_LAYER {
            return self.active;
        }
        if pressed {
            self.held |= 1 << layer;
            self.active = self.shown(layer);
        } else {
            self.held &= !(1 << layer);
            self.active = match self.highest_held() {
                Some(held) => self.shown(held),
                None => self.locked,
            };
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

    const fn shown(self, layer: u8) -> u8 {
        if self.locked == layer { 0 } else { layer }
    }

    const fn highest_held(self) -> Option<u8> {
        let mut layer = TOP_LAYER;
        while layer > 0 {
            if self.held & (1 << layer) != 0 {
                return Some(layer);
            }
            layer -= 1;
        }
        None
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

    #[test]
    fn releasing_one_momentary_layer_returns_to_the_layer_still_held() {
        let mut layers = Layers::new();
        assert_eq!(layers.momentary(1, true), 1);
        assert_eq!(layers.momentary(2, true), 2);
        assert_eq!(layers.momentary(2, false), 1);
        assert_eq!(layers.momentary(1, false), 0);
        assert_eq!(layers.momentary(2, true), 2);
        assert_eq!(layers.momentary(1, true), 1);
        assert_eq!(layers.momentary(1, false), 2);
        assert_eq!(layers.momentary(2, false), 0);
    }

    #[test]
    fn a_held_layer_outlasts_the_peek_below_a_locked_layer() {
        let mut layers = Layers::new();
        assert_eq!(layers.lock(2), Some(2));
        assert_eq!(layers.momentary(2, true), 0);
        assert_eq!(layers.momentary(1, true), 1);
        assert_eq!(layers.momentary(2, false), 1);
        assert_eq!(layers.momentary(1, false), 2);
    }

    #[test]
    fn a_layer_outside_the_layers_is_never_held() {
        let mut layers = Layers::new();
        assert_eq!(layers.momentary(1, true), 1);
        assert_eq!(layers.momentary(4, true), 1);
        assert_eq!(layers.momentary(0, false), 1);
        assert_eq!(layers.momentary(1, false), 0);
        layers.momentary(3, true);
        layers.reset();
        assert_eq!(layers.momentary(2, true), 2);
        assert_eq!(layers.momentary(2, false), 0);
    }
}
