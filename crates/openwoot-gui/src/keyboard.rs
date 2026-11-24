use std::{collections::BTreeSet, rc::Rc};

use gpui_kit::{
    App, ClickEvent, Div, ElementId, Hsla, InteractiveElement as _, IntoElement, Modifiers,
    ParentElement as _, Pixels, RenderOnce, SharedString, StatefulInteractiveElement as _,
    Styled as _, Window, component::ActiveTheme as _, div, prelude::FluentBuilder as _, px,
    relative,
};
use openwoot_protocol::geometry::{self, Shape};

use crate::{store::Keyboard, theme};

const UNIT: f32 = 100.0;
const KEY_GAP: f32 = 4.0;
const CASE_PADDING: u16 = 22;
const FRAME_PIXELS: f32 = KEY_GAP + 2.0;
const DEFAULT_KEY_PIXELS: f32 = 48.0;
const MIN_KEY_PIXELS: f32 = 30.0;
const MAX_KEY_PIXELS: f32 = 60.0;

type KeyClick = Rc<dyn Fn(usize, &ClickEvent, &mut Window, &mut App)>;

#[derive(Debug, Clone, Default)]
pub(crate) struct KeyFace {
    pub(crate) legend: SharedString,
    pub(crate) detail: Option<SharedString>,
    pub(crate) fill: Option<Hsla>,
    pub(crate) travel: Option<f32>,
    pub(crate) dimmed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Selection {
    keys: BTreeSet<usize>,
}

#[derive(IntoElement)]
pub(crate) struct KeyboardView {
    id: ElementId,
    keys: Vec<(usize, geometry::Key, KeyFace)>,
    width: u16,
    height: u16,
    selection: Selection,
    key_pixels: f32,
    on_click: Option<KeyClick>,
}

impl Selection {
    pub(crate) fn click(&mut self, key: usize, modifiers: Modifiers) {
        if modifiers.shift || modifiers.platform || modifiers.control {
            if !self.keys.remove(&key) {
                self.keys.insert(key);
            }
        } else if self.keys.len() == 1 && self.keys.contains(&key) {
            self.keys.clear();
        } else {
            self.keys.clear();
            self.keys.insert(key);
        }
    }

    pub(crate) fn select_all(&mut self, keyboard: &Keyboard) {
        self.keys = (0..keyboard.key_count())
            .filter(|key| keyboard.has_key(*key))
            .collect();
    }

    pub(crate) fn set(&mut self, keys: impl IntoIterator<Item = usize>) {
        self.keys = keys.into_iter().collect();
    }

    pub(crate) fn clear(&mut self) {
        self.keys.clear();
    }

    pub(crate) fn contains(&self, key: usize) -> bool {
        self.keys.contains(&key)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.keys.iter().copied()
    }

    pub(crate) fn first(&self) -> Option<usize> {
        self.keys.first().copied()
    }

    pub(crate) fn len(&self) -> usize {
        self.keys.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

impl KeyboardView {
    pub(crate) fn new(
        id: impl Into<ElementId>,
        keyboard: &Keyboard,
        face: impl Fn(usize) -> KeyFace,
    ) -> Self {
        let keys: Vec<(usize, geometry::Key, KeyFace)> = placements(keyboard)
            .into_iter()
            .map(|(index, key)| (index, key, face(index)))
            .collect();
        let width = keys
            .iter()
            .map(|(_, key, _)| key.x.saturating_add(key.width))
            .max()
            .unwrap_or(0);
        let height = keys
            .iter()
            .map(|(_, key, _)| key.y.saturating_add(key.height))
            .max()
            .unwrap_or(0);
        Self {
            id: id.into(),
            keys,
            width,
            height,
            selection: Selection::default(),
            key_pixels: DEFAULT_KEY_PIXELS,
            on_click: None,
        }
    }

    pub(crate) fn selection(mut self, selection: &Selection) -> Self {
        self.selection = selection.clone();
        self
    }

    pub(crate) fn fit_width(mut self, available: Pixels) -> Self {
        if self.width > 0 {
            let units = f32::from(self.width + 2 * CASE_PADDING) / UNIT;
            let room = f32::from(available) - FRAME_PIXELS;
            self.key_pixels = (room / units).clamp(MIN_KEY_PIXELS, MAX_KEY_PIXELS);
        }
        self
    }

    pub(crate) fn on_key_click(
        mut self,
        handler: impl Fn(usize, &ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    fn scale(&self, hundredths: u16) -> Pixels {
        px(f32::from(hundredths) / UNIT * self.key_pixels)
    }
}

impl RenderOnce for KeyboardView {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let caps = theme::keycaps();
        let outline = theme.primary;
        let hover = theme.secondary_hover;
        let travel_fill = theme.primary.opacity(0.4);
        let gap = px(KEY_GAP);
        let legend_size = px((self.key_pixels * 0.24).clamp(9.0, 13.0));
        let detail_size = px((self.key_pixels * 0.19).clamp(8.0, 11.0));
        let mut plate = div()
            .relative()
            .w(self.scale(self.width))
            .h(self.scale(self.height))
            .flex_none();
        for (index, key, face) in &self.keys {
            let selected = self.selection.contains(*index);
            let top = face.fill.unwrap_or(caps.top);
            let legend = match face.fill {
                _ if face.dimmed => caps.detail.opacity(0.6),
                Some(fill) => readable_on(fill),
                None => caps.legend,
            };
            let detail_color = if face.fill.is_some() {
                legend.opacity(0.85)
            } else {
                caps.detail
            };
            for (part, (x, y, part_width, part_height)) in key_parts(key).into_iter().enumerate() {
                let labelled = part == 0;
                let click = self.on_click.clone();
                let key_index = *index;
                plate = plate.child(
                    div()
                        .id(SharedString::from(format!("key-{index}-{part}")))
                        .absolute()
                        .left(self.scale(x) + gap / 2.0)
                        .top(self.scale(y) + gap / 2.0)
                        .w(self.scale(part_width) - gap)
                        .h(self.scale(part_height) - gap)
                        .rounded(px(4.0))
                        .bg(top)
                        .overflow_hidden()
                        .border_2()
                        .border_color(if selected { outline } else { top })
                        .cursor_pointer()
                        .hover(|style| {
                            if selected {
                                style
                            } else {
                                style.border_color(hover)
                            }
                        })
                        .when_some(face.travel, |this, travel| {
                            this.child(
                                div()
                                    .absolute()
                                    .bottom_0()
                                    .left_0()
                                    .w_full()
                                    .h(relative(travel.clamp(0.0, 1.0)))
                                    .bg(travel_fill),
                            )
                        })
                        .when(labelled, |this| {
                            this.child(legend_layer(
                                face,
                                legend,
                                detail_color,
                                legend_size,
                                detail_size,
                            ))
                        })
                        .when_some(click, move |this, click| {
                            this.on_click(move |event, window, cx| {
                                click(key_index, event, window, cx);
                            })
                        }),
                );
            }
        }
        div()
            .id(self.id.clone())
            .flex_none()
            .rounded(px(16.0))
            .bg(caps.side)
            .pb(px(10.0))
            .shadow_lg()
            .child(
                div()
                    .p(self.scale(CASE_PADDING))
                    .rounded(px(16.0))
                    .bg(caps.case)
                    .border_1()
                    .border_color(caps.plate)
                    .child(div().p(px(KEY_GAP / 2.0)).child(plate)),
            )
    }
}

fn key_parts(key: &geometry::Key) -> Vec<(u16, u16, u16, u16)> {
    match key.shape {
        Shape::Rectangle => vec![(key.x, key.y, key.width, key.height)],
        Shape::IsoEnter => vec![
            (key.x, key.y, key.width, geometry::UNIT),
            (
                key.x.saturating_add(geometry::ISO_ENTER_STEP),
                key.y,
                key.width.saturating_sub(geometry::ISO_ENTER_STEP),
                key.height,
            ),
        ],
    }
}

fn legend_layer(
    face: &KeyFace,
    legend: Hsla,
    detail_color: Hsla,
    legend_size: Pixels,
    detail_size: Pixels,
) -> Div {
    div()
        .absolute()
        .inset_0()
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .text_size(legend_size)
                .text_color(legend)
                .child(face.legend.clone()),
        )
        .when_some(face.detail.clone(), |this, detail| {
            this.child(
                div()
                    .absolute()
                    .top(px(1.0))
                    .left(px(3.0))
                    .text_size(detail_size)
                    .text_color(detail_color)
                    .child(detail),
            )
        })
}

fn placements(keyboard: &Keyboard) -> Vec<(usize, geometry::Key)> {
    if let Some(geometry) = &keyboard.geometry {
        return geometry
            .iter()
            .enumerate()
            .filter_map(|(index, key)| key.map(|key| (index, key)))
            .collect();
    }
    let columns = keyboard.columns().max(1);
    (0..keyboard.key_count())
        .filter(|index| keyboard.has_key(*index))
        .map(|index| {
            let column = u16::try_from(index % columns).unwrap_or(0);
            let row = u16::try_from(index / columns).unwrap_or(0);
            (
                index,
                geometry::Key {
                    x: column.saturating_mul(geometry::UNIT),
                    y: row.saturating_mul(geometry::UNIT),
                    width: geometry::UNIT,
                    height: geometry::UNIT,
                    shape: Shape::Rectangle,
                },
            )
        })
        .collect()
}

fn readable_on(fill: Hsla) -> Hsla {
    if fill.l > 0.6 {
        gpui_kit::black()
    } else {
        gpui_kit::white()
    }
}
