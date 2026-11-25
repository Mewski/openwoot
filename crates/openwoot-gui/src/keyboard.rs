use std::{collections::BTreeSet, fmt, rc::Rc};

use gpui_kit::{
    AnyElement, App, AppContext as _, ClickEvent, Context, Div, ElementId, Entity, FontWeight,
    Hsla, InteractiveElement as _, IntoElement, MouseButton, MouseMoveEvent, ParentElement as _,
    Pixels, Render, RenderOnce, SharedString, Size, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, component::ActiveTheme as _, div, linear_color_stop, linear_gradient,
    prelude::FluentBuilder as _, px, relative,
};
use openwoot_protocol::geometry::{self, Shape};

use crate::{
    panel::{Tip, raised, tip_surface, with_tip},
    store::Keyboard,
    theme,
};

const UNIT: f32 = 100.0;
const CASE_TOP: u16 = 80;
const CASE_SIDE: u16 = 50;
const CASE_LEFT: u16 = 40;
const CASE_RIGHT: u16 = 48;
const CASE_BOTTOM: u16 = 45;
const CASE_RADIUS: f32 = 0.27;
const CASE_LIP: f32 = 2.0;
const KEY_INSET: f32 = 1.0;
const KEY_RADIUS: f32 = 0.075;
const LEGEND_SHARE: f32 = 0.25;
const DETAIL_SHARE: f32 = 0.19;
const FILL_OPACITY: f32 = 0.7;
const HOVER_FILL_OPACITY: f32 = 0.5;
const DEFAULT_KEY_PIXELS: f32 = 44.0;
const MIN_KEY_PIXELS: f32 = 16.0;
const DRAG_PREVIEW_PIXELS: f32 = 44.0;
const MARKER_PIXELS: f32 = 14.0;
const MARKER_INSET: f32 = 3.0;
const CALLOUT_HEIGHT: f32 = 39.0;
const CALLOUT_GAP: f32 = 7.0;

type KeyClick = Rc<dyn Fn(usize, &ClickEvent, &mut Window, &mut App)>;
type KeyAction = Rc<dyn Fn(usize, &mut Window, &mut App)>;
type KeyDecorator = Rc<dyn Fn(Stateful<Div>, usize, Hsla) -> Stateful<Div>>;
type MarkerPress = Rc<dyn Fn(usize, MouseButton, &mut Window, &mut App)>;

#[derive(Debug, Clone, Default)]
pub(crate) struct KeyFace {
    pub(crate) legend: SharedString,
    pub(crate) detail: Option<SharedString>,
    pub(crate) fill: Option<Hsla>,
    pub(crate) travel: Option<f32>,
    pub(crate) dimmed: bool,
    pub(crate) glyph: Option<Glyph>,
    pub(crate) tooltip: Option<SharedString>,
    pub(crate) marker: Option<Hsla>,
    pub(crate) corner: Option<SharedString>,
    pub(crate) lower_fill: Option<Hsla>,
    pub(crate) callout: Option<SharedString>,
    pub(crate) hover_glyph: Option<Glyph>,
}

#[derive(Clone)]
pub(crate) struct Glyph(Rc<dyn Fn(Pixels) -> AnyElement>);

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Selection {
    keys: BTreeSet<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct Dragged<T> {
    value: T,
    label: SharedString,
    glyph: Option<Glyph>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Sweep {
    start: Option<usize>,
    last: Option<usize>,
    swept: bool,
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
    on_sweep: Option<KeyAction>,
    on_secondary: Option<KeyAction>,
    decorators: Vec<KeyDecorator>,
    on_marker: Option<MarkerPress>,
    raised: bool,
}

impl Selection {
    pub(crate) fn toggle(&mut self, key: usize) {
        if !self.keys.remove(&key) {
            self.keys.insert(key);
        }
    }

    pub(crate) fn insert(&mut self, key: usize) {
        self.keys.insert(key);
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
            on_sweep: None,
            on_secondary: None,
            decorators: Vec::new(),
            on_marker: None,
            raised: false,
        }
    }

    pub(crate) fn selection(mut self, selection: &Selection) -> Self {
        self.selection = selection.clone();
        self
    }

    pub(crate) fn raise_selection(mut self) -> Self {
        self.raised = true;
        self
    }

    pub(crate) fn fit(mut self, room: Size<Pixels>) -> Self {
        if self.width > 0 && self.height > 0 {
            let across = f32::from(self.width + 2 * CASE_SIDE) / UNIT;
            let down = f32::from(self.height + CASE_TOP + CASE_BOTTOM) / UNIT;
            let pixels = (f32::from(room.width) / across).min(f32::from(room.height) / down);
            self.key_pixels = pixels.max(MIN_KEY_PIXELS);
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

    pub(crate) fn on_key_sweep(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sweep = Some(Rc::new(handler));
        self
    }

    pub(crate) fn on_key_secondary_click(
        mut self,
        handler: impl Fn(usize, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_secondary = Some(Rc::new(handler));
        self
    }

    pub(crate) fn on_marker_press(
        mut self,
        handler: impl Fn(usize, MouseButton, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_marker = Some(Rc::new(handler));
        self
    }

    pub(crate) fn on_key_drag<T: Clone + 'static>(
        mut self,
        payload: impl Fn(usize) -> Option<Dragged<T>> + 'static,
    ) -> Self {
        self.decorators.push(Rc::new(move |element, key, _| {
            let Some(dragged) = payload(key) else {
                return element;
            };
            element.on_drag(dragged, |dragged, _, _, cx| cx.new(|_| dragged.clone()))
        }));
        self
    }

    pub(crate) fn on_key_drop<T: 'static>(
        mut self,
        handler: impl Fn(usize, &T, &mut Window, &mut App) + 'static,
    ) -> Self {
        let handler = Rc::new(handler);
        self.decorators
            .push(Rc::new(move |element, key, highlight| {
                let handler = handler.clone();
                element
                    .drag_over::<Dragged<T>>(move |style, _, _, _| {
                        style.border_color(highlight).bg(highlight.opacity(0.3))
                    })
                    .on_drop(move |dragged: &Dragged<T>, window, cx| {
                        handler(key, &dragged.value, window, cx);
                    })
            }));
        self
    }

    fn scale(&self, hundredths: u16) -> Pixels {
        px(f32::from(hundredths) / UNIT * self.key_pixels)
    }

    fn interactive(
        &self,
        element: Stateful<Div>,
        key: usize,
        sweep: &Entity<Sweep>,
        highlight: Hsla,
    ) -> Stateful<Div> {
        let mut element = element
            .when_some(self.on_click.clone(), |this, click| {
                let sweep = sweep.clone();
                this.on_click(move |event, window, cx| {
                    if !sweep.read(cx).swept {
                        click(key, event, window, cx);
                    }
                })
            })
            .when_some(self.on_sweep.clone(), |this, on_sweep| {
                let pressed = sweep.clone();
                let moved = sweep.clone();
                this.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    pressed.update(cx, |sweep, _| {
                        *sweep = Sweep {
                            start: Some(key),
                            last: Some(key),
                            swept: false,
                        };
                    });
                })
                .on_mouse_move(move |event, window, cx| {
                    sweep_over(&moved, key, event, &on_sweep, window, cx);
                })
            })
            .when_some(self.on_secondary.clone(), |this, on_secondary| {
                this.on_mouse_down(MouseButton::Right, move |_, window, cx| {
                    cx.stop_propagation();
                    on_secondary(key, window, cx);
                })
            });
        for decorate in &self.decorators {
            element = decorate(element, key, highlight);
        }
        element
    }

    fn marker(&self, key: usize, color: Hsla, ring: Hsla) -> Stateful<Div> {
        let empty = color.a <= f32::EPSILON;
        div()
            .id(SharedString::from(format!("key-marker-{key}")))
            .absolute()
            .top(px(MARKER_INSET))
            .right(px(MARKER_INSET))
            .size(px(MARKER_PIXELS))
            .rounded_full()
            .border_1()
            .border_color(if empty { readable_on(ring) } else { ring })
            .bg(color)
            .when_some(self.on_marker.clone(), |this, on_marker| {
                let secondary = on_marker.clone();
                this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    on_marker(key, MouseButton::Left, window, cx);
                })
                .on_mouse_down(MouseButton::Right, move |_, window, cx| {
                    cx.stop_propagation();
                    secondary(key, MouseButton::Right, window, cx);
                })
            })
    }

    fn case(&self, plate: Div, caps: theme::Keycaps) -> Stateful<Div> {
        let radius = px(self.key_pixels * CASE_RADIUS);
        let rim = px(1.0);
        div()
            .id(self.id.clone())
            .flex_none()
            .pl(self.scale(CASE_SIDE - CASE_LEFT))
            .pr(self.scale(CASE_SIDE - CASE_RIGHT))
            .child(
                div().rounded(radius).bg(caps.lip).pb(px(CASE_LIP)).child(
                    div()
                        .rounded(radius)
                        .bg(linear_gradient(
                            180.0,
                            linear_color_stop(caps.case_top, 0.0),
                            linear_color_stop(caps.case_bottom, 1.0),
                        ))
                        .border_1()
                        .border_color(caps.rim)
                        .pt(self.scale(CASE_TOP) - rim)
                        .pl(self.scale(CASE_LEFT) - rim)
                        .pr(self.scale(CASE_RIGHT) - rim)
                        .pb(self.scale(CASE_BOTTOM) - rim - px(CASE_LIP))
                        .child(plate),
                ),
            )
    }

    fn callout(&self, key: &geometry::Key, text: SharedString, cx: &App) -> Div {
        div()
            .absolute()
            .left(self.scale(key.x))
            .top(self.scale(key.y) - px(CALLOUT_HEIGHT + CALLOUT_GAP))
            .w(self.scale(key.width))
            .h(px(CALLOUT_HEIGHT))
            .flex()
            .justify_center()
            .child(div().flex_none().child(tip_surface(Tip::Text(text), cx)))
    }
}

impl RenderOnce for KeyboardView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let sweep = window.use_keyed_state(self.id.clone(), cx, |_, _| Sweep::default());
        let theme = cx.theme();
        let caps = theme::keycaps();
        let outline = theme.primary;
        let travel_fill = theme.primary.opacity(0.4);
        let inset = px(KEY_INSET);
        let radius = px(self.key_pixels * KEY_RADIUS);
        let legend_size = px(self.key_pixels * LEGEND_SHARE);
        let detail_size = px(self.key_pixels * DETAIL_SHARE);
        let mut plate = div()
            .relative()
            .w(self.scale(self.width))
            .h(self.scale(self.height))
            .flex_none();
        for (index, key, face) in &self.keys {
            let selected = self.selection.contains(*index);
            let (top, hovered_top) = match face.fill {
                Some(fill) => (fill.opacity(FILL_OPACITY), fill.opacity(HOVER_FILL_OPACITY)),
                None => (caps.top, caps.top),
            };
            let legend = match face.fill {
                _ if face.dimmed => caps.detail.opacity(0.6),
                Some(fill) => readable_on(blend(fill, caps.case_top)),
                None => caps.legend,
            };
            let detail_color = if face.fill.is_some() {
                legend.opacity(0.85)
            } else {
                caps.detail
            };
            let border = if selected { outline } else { caps.border };
            for (part, (x, y, part_width, part_height)) in key_parts(key).into_iter().enumerate() {
                let labelled = part == 0;
                let key_index = *index;
                let group = SharedString::from(format!("key-{index}-{part}"));
                let element = div()
                    .group(group.clone())
                    .id(group.clone())
                    .absolute()
                    .left(self.scale(x) + inset)
                    .top(self.scale(y) + inset)
                    .w(self.scale(part_width) - inset * 2.0)
                    .h(self.scale(part_height) - inset * 2.0)
                    .rounded(radius)
                    .bg(top)
                    .overflow_hidden()
                    .border_1()
                    .border_color(border)
                    .cursor_pointer()
                    .hover(|style| {
                        if selected {
                            style.bg(hovered_top)
                        } else {
                            style.bg(hovered_top).border_color(caps.border_hover)
                        }
                    })
                    .children(fills(face, travel_fill))
                    .when(labelled, |this| {
                        this.child(legend_layer(
                            face,
                            legend,
                            detail_color,
                            (legend_size, detail_size),
                            (self.scale(part_height) - inset * 2.0, group),
                        ))
                    })
                    .when_some(face.marker.filter(|_| labelled), |this, color| {
                        this.child(self.marker(key_index, color, caps.case_bottom))
                    });
                let element = match face.tooltip.clone().filter(|_| labelled) {
                    Some(text) => with_tip(
                        element,
                        SharedString::from(format!("key-tip-{index}")),
                        Tip::Text(text),
                        window,
                        cx,
                    ),
                    None => element,
                };
                let element = self.interactive(element, key_index, &sweep, outline);
                plate = if self.raised && selected {
                    plate.child(raised(element))
                } else {
                    plate.child(element)
                };
            }
        }
        let callouts = self.keys.iter().filter_map(|(_, key, face)| {
            let text = face.callout.clone()?;
            Some(self.callout(key, text, cx))
        });
        self.case(plate.children(callouts), caps)
    }
}

impl Glyph {
    pub(crate) fn new(draw: impl Fn(Pixels) -> AnyElement + 'static) -> Self {
        Self(Rc::new(draw))
    }

    fn draw(&self, height: Pixels) -> AnyElement {
        (self.0)(height)
    }
}

impl fmt::Debug for Glyph {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Glyph")
    }
}

impl<T> Dragged<T> {
    pub(crate) fn new(value: T, label: impl Into<SharedString>) -> Self {
        Self {
            value,
            label: label.into(),
            glyph: None,
        }
    }

    pub(crate) fn glyph(mut self, glyph: Glyph) -> Self {
        self.glyph = Some(glyph);
        self
    }

    pub(crate) fn value(&self) -> &T {
        &self.value
    }
}

impl<T: 'static> Render for Dragged<T> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let caps = theme::keycaps();
        let preview = div()
            .min_w(px(DRAG_PREVIEW_PIXELS))
            .h(px(DRAG_PREVIEW_PIXELS))
            .flex()
            .items_center()
            .justify_center();
        match &self.glyph {
            Some(glyph) => preview.child(glyph.draw(px(DRAG_PREVIEW_PIXELS))),
            None => preview
                .px_2()
                .rounded(px(4.0))
                .bg(caps.top)
                .border_2()
                .border_color(cx.theme().primary)
                .shadow_lg()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(caps.legend)
                .child(self.label.clone()),
        }
    }
}

fn sweep_over(
    sweep: &Entity<Sweep>,
    key: usize,
    event: &MouseMoveEvent,
    on_sweep: &KeyAction,
    window: &mut Window,
    cx: &mut App,
) {
    let state = *sweep.read(cx);
    if event.pressed_button != Some(MouseButton::Left) || cx.has_active_drag() {
        if state.start.is_some() {
            sweep.update(cx, |sweep, _| {
                sweep.start = None;
                sweep.last = None;
            });
        }
        return;
    }
    let Some(start) = state.start else {
        return;
    };
    if state.last == Some(key) {
        return;
    }
    sweep.update(cx, |sweep, _| {
        sweep.last = Some(key);
        sweep.swept = true;
    });
    if !state.swept {
        on_sweep(start, window, cx);
    }
    on_sweep(key, window, cx);
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
    (legend_size, detail_size): (Pixels, Pixels),
    (face_height, group): (Pixels, SharedString),
) -> Div {
    let centre = || {
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
    };
    let hovered = face.hover_glyph.is_some();
    let resting = match &face.glyph {
        Some(glyph) => centre().child(glyph.draw(face_height)),
        None => centre()
            .text_size(legend_size)
            .text_color(legend)
            .child(face.legend.clone()),
    }
    .when(hovered, |this| {
        this.group_hover(group.clone(), |style| style.opacity(0.0))
    });
    div()
        .absolute()
        .inset_0()
        .child(resting)
        .when_some(face.hover_glyph.as_ref(), |this, glyph| {
            this.child(
                centre()
                    .opacity(0.0)
                    .group_hover(group, |style| style.opacity(1.0))
                    .child(glyph.draw(face_height)),
            )
        })
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
        .when_some(face.corner.clone(), |this, corner| {
            this.child(
                div()
                    .absolute()
                    .bottom(px(1.0))
                    .right(px(3.0))
                    .text_size(detail_size)
                    .text_color(detail_color)
                    .child(corner),
            )
        })
}

fn fills(face: &KeyFace, travel_fill: Hsla) -> Vec<Div> {
    let layer = |height: f32, color: Hsla| {
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .w_full()
            .h(relative(height))
            .bg(color)
    };
    face.lower_fill
        .map(|lower| layer(0.5, lower))
        .into_iter()
        .chain(
            face.travel
                .map(|travel| layer(travel.clamp(0.0, 1.0), travel_fill)),
        )
        .collect()
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

fn blend(fill: Hsla, under: Hsla) -> Hsla {
    Hsla {
        l: fill.l * FILL_OPACITY + under.l * (1.0 - FILL_OPACITY),
        ..fill
    }
}
