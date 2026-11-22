use super::{
    canvas::Canvas,
    color::BLACK,
    config::{KeyColors, Rgb},
};

pub const LAYER_COLOR_MAPS: usize = 3;

pub type KeyColorMap<const ROWS: usize, const COLUMNS: usize> = [[Rgb; COLUMNS]; ROWS];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct LayerColors<const ROWS: usize, const COLUMNS: usize> {
    base: KeyColorMap<ROWS, COLUMNS>,
    layers: [Option<KeyColorMap<ROWS, COLUMNS>>; LAYER_COLOR_MAPS],
    shown: u8,
    requested: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shown<const ROWS: usize, const COLUMNS: usize> {
    Layer(KeyColorMap<ROWS, COLUMNS>),
    Base,
}

impl<const ROWS: usize, const COLUMNS: usize> LayerColors<ROWS, COLUMNS> {
    pub(super) const fn new() -> Self {
        Self {
            base: [[BLACK; COLUMNS]; ROWS],
            layers: [None; LAYER_COLOR_MAPS],
            shown: 0,
            requested: None,
        }
    }

    pub(super) const fn base(&self) -> &KeyColorMap<ROWS, COLUMNS> {
        &self.base
    }

    pub(super) const fn shown_layer(&self) -> u8 {
        self.shown
    }

    pub(super) fn shown(&self) -> &KeyColorMap<ROWS, COLUMNS> {
        self.layer(self.shown).unwrap_or(&self.base)
    }

    pub(super) fn set_base(&mut self, colors: &KeyColors) {
        self.base = color_map(colors);
    }

    pub(super) fn set_layer(&mut self, layer: u8, colors: Option<&KeyColors>) {
        let Some(slot) = usize::from(layer)
            .checked_sub(1)
            .and_then(|index| self.layers.get_mut(index))
        else {
            return;
        };
        *slot = colors.map(color_map);
        if self.shown == layer {
            self.requested = Some(layer);
        }
    }

    pub(super) const fn request(&mut self, layer: u8) {
        self.requested = Some(layer);
    }

    pub(super) const fn hide_layers(&mut self) {
        self.shown = 0;
        self.requested = None;
    }

    pub(super) fn take_request(&mut self) -> Option<Shown<ROWS, COLUMNS>> {
        let layer = self.requested.take()?;
        if let Some(colors) = self.layer(layer).copied() {
            self.shown = layer;
            return Some(Shown::Layer(colors));
        }
        if self.shown == 0 {
            return None;
        }
        self.shown = 0;
        Some(Shown::Base)
    }

    fn layer(&self, layer: u8) -> Option<&KeyColorMap<ROWS, COLUMNS>> {
        usize::from(layer)
            .checked_sub(1)
            .and_then(|index| self.layers.get(index))
            .and_then(Option::as_ref)
    }
}

pub(super) fn paint<const ROWS: usize, const COLUMNS: usize>(
    canvas: &mut impl Canvas,
    colors: &KeyColorMap<ROWS, COLUMNS>,
) {
    for (row, colors) in colors.iter().enumerate() {
        for (column, &color) in colors.iter().enumerate() {
            canvas.set_key(row, column, color);
        }
    }
}

pub(super) fn color_map<const ROWS: usize, const COLUMNS: usize>(
    colors: &KeyColors,
) -> KeyColorMap<ROWS, COLUMNS> {
    let mut map = [[BLACK; COLUMNS]; ROWS];
    for (index, slot) in map.iter_mut().flatten().enumerate() {
        *slot = colors.get(index).copied().unwrap_or(BLACK);
    }
    map
}
