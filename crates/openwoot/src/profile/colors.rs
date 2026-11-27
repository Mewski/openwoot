use openwoot_protocol::{
    keys::Layer,
    lighting::KeyColors,
    wire::{Decode, Encode},
};

use crate::layout::LAYERS;

#[derive(Clone, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct Colors(pub [Option<KeyColors>; LAYERS]);

impl Colors {
    #[must_use]
    pub fn layer(&self, layer: Layer) -> Option<&KeyColors> {
        self.0
            .get(usize::from(layer.number()))
            .and_then(Option::as_ref)
    }

    pub fn set(&mut self, layer: Layer, colors: Option<KeyColors>) {
        if let Some(slot) = self.0.get_mut(usize::from(layer.number())) {
            *slot = colors;
        }
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::lighting::Rgb;

    use super::*;

    #[test]
    fn each_layer_keeps_its_own_map() {
        let mut colors = Colors::default();
        let red: KeyColors = [Rgb {
            red: 255,
            green: 0,
            blue: 0,
        }]
        .into_iter()
        .collect();
        colors.set(Layer::One, Some(red.clone()));
        assert_eq!(colors.layer(Layer::One), Some(&red));
        assert_eq!(colors.layer(Layer::Base), None);
        colors.set(Layer::One, None);
        assert_eq!(colors, Colors::default());
    }
}
