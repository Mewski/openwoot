use std::{cell::Cell, mem, rc::Rc};

use gpui_kit::{
    AnyElement, App, AppContext as _, Bounds, ClipboardItem, Context, Div, DragMoveEvent,
    ElementId, Empty, Entity, EventEmitter, Global, Hsla, InteractiveElement as _, IntoElement,
    MouseButton, MouseDownEvent, ParentElement as _, Pixels, Point, Render, RenderOnce, Rgba,
    Stateful, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    base::actions::Confirm,
    canvas,
    component::{
        ActiveTheme as _, Icon, Selectable as _, Sizable as _, WindowExt as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        input::{Input, InputEvent, InputState},
        notification::Notification,
        v_flex,
    },
    div, hsla, linear_color_stop, linear_gradient,
    prelude::FluentBuilder as _,
    px, relative,
};
use openwoot_protocol::lighting::Rgb;

use crate::{
    dropdown::{Dropdown, Item},
    panel,
};

pub(crate) const POPOVER_WIDTH: f32 = 288.0;

const AREA_HEIGHT: f32 = 200.0;
const AREA_KNOB: f32 = 14.0;
const STRIP_HEIGHT: f32 = 12.0;
const STRIP_KNOB: f32 = 18.0;
const SWATCH_SIZE: f32 = 32.0;
const SWATCHES_PER_ROW: usize = 6;
const RECENT_LIMIT: usize = 12;
const MODE_WIDTH: f32 = 58.0;
const CHANNEL_MIN_WIDTH: f32 = 56.0;
const MODE_MENU_WIDTH: f32 = 160.0;
const FIELD_HEIGHT: f32 = 30.0;
const HUE_SEGMENTS: u8 = 6;
const CHANNEL_MAX: f32 = 255.0;
const CHANNEL_NAMES: [&str; 3] = ["R", "G", "B"];
const WHITE: Rgb = color(255, 255, 255);
const DEFAULT_RECENT: [Rgb; RECENT_LIMIT] = [
    color(0, 0, 0),
    color(0, 0, 255),
    color(0, 255, 255),
    color(0, 255, 0),
    color(255, 255, 0),
    color(255, 0, 0),
    color(255, 0, 255),
    WHITE,
    color(255, 179, 186),
    color(186, 255, 201),
    color(204, 236, 255),
    color(255, 255, 186),
];

pub(crate) struct ColorPicker {
    value: Option<Rgb>,
    hsv: Hsv,
    allow_none: bool,
    pipette: Option<bool>,
    channels: [Entity<InputState>; 3],
    hex: Entity<InputState>,
    area: Rc<Cell<Bounds<Pixels>>>,
    strip: Rc<Cell<Bounds<Pixels>>>,
    pressed: bool,
    uncommitted: bool,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorPickerEvent {
    Change(Option<Rgb>),
    Pipette,
}

#[derive(IntoElement)]
pub(crate) struct Swatch {
    id: ElementId,
    picker: Entity<ColorPicker>,
    size: Pixels,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Rgb,
    Hex,
}

struct Memory {
    recent: Vec<Rgb>,
    mode: Mode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Hsv {
    hue: f32,
    saturation: f32,
    value: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Pointer,
    Channels,
    Hex,
    Other,
}

struct AreaDrag;

struct StripDrag;

impl EventEmitter<ColorPickerEvent> for ColorPicker {}

impl Global for Memory {}

impl ColorPicker {
    pub(crate) fn new(
        value: impl Into<Option<Rgb>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let value = value.into();
        let hsv = Hsv::of(value.unwrap_or(WHITE));
        let shown = hsv.rgb();
        let channels = [shown.red, shown.green, shown.blue].map(|channel| {
            cx.new(|cx| InputState::new(window, cx).default_value(channel.to_string()))
        });
        let hex = cx.new(|cx| InputState::new(window, cx).default_value(hex_of(shown)));
        let mut subscriptions = vec![cx.observe_global::<Memory>(|_, cx| cx.notify())];
        for channel in &channels {
            subscriptions.push(cx.subscribe_in(
                channel,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => this.type_channels(window, cx),
                    InputEvent::PressEnter { .. } | InputEvent::Blur => this.settle(window, cx),
                    InputEvent::Focus => {}
                },
            ));
        }
        subscriptions.push(cx.subscribe_in(
            &hex,
            window,
            |this, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => this.type_hex(window, cx),
                InputEvent::PressEnter { .. } | InputEvent::Blur => this.settle(window, cx),
                InputEvent::Focus => {}
            },
        ));
        Self {
            value,
            hsv,
            allow_none: false,
            pipette: None,
            channels,
            hex,
            area: Rc::default(),
            strip: Rc::default(),
            pressed: false,
            uncommitted: false,
            _subscriptions: subscriptions,
        }
    }

    #[must_use]
    pub(crate) const fn allow_no_color(mut self) -> Self {
        self.allow_none = true;
        self
    }

    pub(crate) const fn value(&self) -> Option<Rgb> {
        self.value
    }

    pub(crate) fn set_value(
        &mut self,
        value: impl Into<Option<Rgb>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = value.into();
        if let Some(color) = value {
            self.hsv = self.hsv.adopt(color);
            self.write_fields(color, Source::Other, window, cx);
        }
        self.value = value;
        self.uncommitted = false;
        cx.notify();
    }

    pub(crate) fn set_pipette(&mut self, pipette: Option<bool>, cx: &mut Context<Self>) {
        if self.pipette != pipette {
            self.pipette = pipette;
            cx.notify();
        }
    }

    pub(crate) fn remember(color: Rgb, cx: &mut App) {
        cx.default_global::<Memory>().remember(color);
    }

    fn change(&mut self, value: Option<Rgb>, cx: &mut Context<Self>) {
        if self.value != value {
            self.value = value;
            cx.emit(ColorPickerEvent::Change(value));
        }
        cx.notify();
    }

    fn point(&mut self, hsv: Hsv, window: &mut Window, cx: &mut Context<Self>) {
        self.hsv = hsv;
        let color = hsv.rgb();
        self.write_fields(color, Source::Pointer, window, cx);
        self.uncommitted = true;
        self.change(Some(color), cx);
    }

    fn drag_area(
        &mut self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let hsv = Hsv {
            saturation: fraction(position.x - bounds.origin.x, bounds.size.width),
            value: 1.0 - fraction(position.y - bounds.origin.y, bounds.size.height),
            ..self.hsv
        };
        self.point(hsv, window, cx);
    }

    fn drag_strip(
        &mut self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let hsv = Hsv {
            hue: fraction(position.x - bounds.origin.x, bounds.size.width),
            ..self.hsv
        };
        self.point(hsv, window, cx);
    }

    fn release(&mut self, cx: &mut Context<Self>) {
        if mem::take(&mut self.pressed) {
            self.commit(cx);
        }
    }

    fn commit(&mut self, cx: &mut Context<Self>) {
        if mem::take(&mut self.uncommitted)
            && let Some(color) = self.value
        {
            Self::remember(color, cx);
        }
    }

    fn type_channels(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let [red, green, blue] = self
            .channels
            .each_ref()
            .map(|state| state.read(cx).value().trim().parse::<u8>().ok());
        if let (Some(red), Some(green), Some(blue)) = (red, green, blue) {
            self.typed(Rgb { red, green, blue }, Source::Channels, window, cx);
        }
    }

    fn type_hex(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(color) = parse_hex(&self.hex.read(cx).value()) {
            self.typed(color, Source::Hex, window, cx);
        }
    }

    fn typed(&mut self, color: Rgb, source: Source, window: &mut Window, cx: &mut Context<Self>) {
        if self.value == Some(color) {
            return;
        }
        self.hsv = self.hsv.adopt(color);
        self.write_fields(color, source, window, cx);
        self.uncommitted = true;
        self.change(Some(color), cx);
    }

    fn settle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(color) = self.value {
            self.write_fields(color, Source::Other, window, cx);
        }
        self.commit(cx);
    }

    fn choose(&mut self, color: Rgb, window: &mut Window, cx: &mut Context<Self>) {
        self.hsv = self.hsv.adopt(color);
        self.write_fields(color, Source::Other, window, cx);
        self.change(Some(color), cx);
    }

    fn write_fields(&self, color: Rgb, source: Source, window: &mut Window, cx: &mut App) {
        if source != Source::Channels {
            for (state, channel) in self
                .channels
                .iter()
                .zip([color.red, color.green, color.blue])
            {
                state.update(cx, |input, cx| {
                    input.set_value(channel.to_string(), window, cx);
                });
            }
        }
        if source != Source::Hex {
            self.hex
                .update(cx, |input, cx| input.set_value(hex_of(color), window, cx));
        }
    }

    fn copy_hex(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(color) = self.value {
            cx.write_to_clipboard(ClipboardItem::new_string(hex_of(color)));
            window.push_notification(Notification::success("Colour copied"), cx);
        }
    }

    fn area(&self, cx: &Context<Self>) -> Stateful<Div> {
        let radius = cx.theme().radius;
        let white = hsla(0.0, 0.0, 1.0, 1.0);
        let black = hsla(0.0, 0.0, 0.0, 1.0);
        let bounds = Rc::clone(&self.area);
        let layer = || {
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .rounded(radius)
        };
        let knob = self.value.map(|color| {
            div()
                .absolute()
                .left(relative(self.hsv.saturation))
                .top(relative(1.0 - self.hsv.value))
                .ml(px(-AREA_KNOB / 2.0))
                .mt(px(-AREA_KNOB / 2.0))
                .size(px(AREA_KNOB))
                .rounded_full()
                .border_2()
                .border_color(white)
                .bg(hsla_of(color))
        });
        div()
            .id("color-area")
            .relative()
            .w_full()
            .h(px(AREA_HEIGHT))
            .rounded(radius)
            .cursor_crosshair()
            .bg(hsla(self.hsv.hue, 1.0, 0.5, 1.0))
            .child(layer().bg(linear_gradient(
                90.0,
                linear_color_stop(white, 0.0),
                linear_color_stop(white.opacity(0.0), 1.0),
            )))
            .child(layer().bg(linear_gradient(
                180.0,
                linear_color_stop(black.opacity(0.0), 0.0),
                linear_color_stop(black, 1.0),
            )))
            .children(knob)
            .child(
                canvas(move |area, _, _| bounds.set(area), |_, (), _, _| {})
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.pressed = true;
                    let bounds = this.area.get();
                    this.drag_area(event.position, bounds, window, cx);
                }),
            )
            .on_drag(AreaDrag, |_, _, _, cx| cx.new(|_| AreaDrag))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<AreaDrag>, window, cx| {
                    this.drag_area(event.event.position, event.bounds, window, cx);
                }),
            )
            .on_drop(cx.listener(|this, _: &AreaDrag, _, cx| this.release(cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.release(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.release(cx)),
            )
    }

    fn hue_row(&self, cx: &Context<Self>) -> Div {
        let pipette = self.pipette.map(|active| {
            Button::new("color-pipette")
                .ghost()
                .small()
                .icon(Catalog::Pipette)
                .selected(active)
                .tooltip("Eye dropper")
                .on_click(cx.listener(|_, _, _, cx| cx.emit(ColorPickerEvent::Pipette)))
        });
        h_flex()
            .w_full()
            .gap_2()
            .children(pipette)
            .child(self.strip(cx))
    }

    fn strip(&self, cx: &Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let bounds = Rc::clone(&self.strip);
        let segments = (0..HUE_SEGMENTS).map(|segment| {
            let start = f32::from(segment) / f32::from(HUE_SEGMENTS);
            let end = (f32::from(segment) + 1.0) / f32::from(HUE_SEGMENTS);
            div().flex_1().h_full().bg(linear_gradient(
                90.0,
                linear_color_stop(hsla(start, 1.0, 0.5, 1.0), 0.0),
                linear_color_stop(hsla(end.rem_euclid(1.0), 1.0, 0.5, 1.0), 1.0),
            ))
        });
        div()
            .id("color-hue")
            .relative()
            .flex_1()
            .h(px(STRIP_KNOB))
            .cursor_pointer()
            .child(
                h_flex()
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(px((STRIP_KNOB - STRIP_HEIGHT) / 2.0))
                    .h(px(STRIP_HEIGHT))
                    .rounded_full()
                    .overflow_hidden()
                    .children(segments),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(relative(self.hsv.hue))
                    .ml(px(-STRIP_KNOB / 2.0))
                    .size(px(STRIP_KNOB))
                    .rounded_full()
                    .border_1()
                    .border_color(theme.border)
                    .bg(hsla(0.0, 0.0, 1.0, 1.0))
                    .shadow_sm(),
            )
            .child(
                canvas(move |strip, _, _| bounds.set(strip), |_, (), _, _| {})
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    this.pressed = true;
                    let bounds = this.strip.get();
                    this.drag_strip(event.position, bounds, window, cx);
                }),
            )
            .on_drag(StripDrag, |_, _, _, cx| cx.new(|_| StripDrag))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<StripDrag>, window, cx| {
                    this.drag_strip(event.event.position, event.bounds, window, cx);
                }),
            )
            .on_drop(cx.listener(|this, _: &StripDrag, _, cx| this.release(cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.release(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.release(cx)),
            )
    }

    fn fields(&self, mode: Mode, cx: &Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        let modes = Dropdown::new("color-mode")
            .menu_width(px(MODE_MENU_WIDTH))
            .items(Mode::ALL.map(|entry| Item::new(entry.label())))
            .selected(Mode::ALL.iter().position(|entry| *entry == mode))
            .on_select(|index, _, cx| {
                if let Some(mode) = Mode::ALL.get(index) {
                    cx.default_global::<Memory>().mode = *mode;
                }
            })
            .trigger(move |open, cx| mode_face(mode, open, cx));
        let inputs = match mode {
            Mode::Rgb => h_flex().flex_1().min_w_0().gap_1().children(
                self.channels
                    .iter()
                    .zip(CHANNEL_NAMES)
                    .map(|(state, name)| {
                        div().flex_1().min_w(px(CHANNEL_MIN_WIDTH)).child(
                            Input::new(state)
                                .small()
                                .px_1p5()
                                .prefix(div().text_xs().text_color(muted).child(name)),
                        )
                    }),
            ),
            Mode::Hex => h_flex().flex_1().min_w_0().child(
                div().flex_1().min_w_0().child(
                    Input::new(&self.hex).small().suffix(
                        Button::new("color-copy")
                            .ghost()
                            .xsmall()
                            .icon(Catalog::Copy)
                            .tooltip("Copy")
                            .on_click(cx.listener(|this, _, window, cx| this.copy_hex(window, cx))),
                    ),
                ),
            ),
        };
        h_flex()
            .w_full()
            .gap_1p5()
            .on_action(|_: &Confirm, _, _| {})
            .child(div().flex_none().w(px(MODE_WIDTH)).child(modes))
            .child(inputs)
    }

    fn recent(&self, recent: &[Rgb], cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let rows = recent
            .chunks(SWATCHES_PER_ROW)
            .enumerate()
            .map(|(row, colors)| {
                h_flex()
                    .w_full()
                    .justify_between()
                    .children(colors.iter().enumerate().map(|(column, color)| {
                        let color = *color;
                        div()
                            .id(("recent-color", row * SWATCHES_PER_ROW + column))
                            .flex_none()
                            .size(px(SWATCH_SIZE))
                            .p(px(2.0))
                            .rounded_full()
                            .border_2()
                            .border_color(if self.value == Some(color) {
                                theme.primary
                            } else {
                                theme.transparent
                            })
                            .cursor_pointer()
                            .child(
                                div()
                                    .size_full()
                                    .rounded_full()
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(hsla_of(color)),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.choose(color, window, cx);
                            }))
                    }))
            });
        v_flex()
            .w_full()
            .gap_3()
            .child(panel::label("Recently used", cx))
            .child(v_flex().w_full().gap_3().children(rows))
    }

    fn no_color(&self, cx: &Context<Self>) -> Stateful<Div> {
        let theme = cx.theme();
        let chosen = self.value.is_none();
        h_flex()
            .id("no-color")
            .w_full()
            .gap_3()
            .cursor_pointer()
            .child(
                div()
                    .flex_none()
                    .size(px(SWATCH_SIZE))
                    .rounded_full()
                    .border_2()
                    .border_color(if chosen { theme.primary } else { theme.border })
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(Catalog::Ban)
                            .small()
                            .text_color(theme.muted_foreground),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(if chosen {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .child("No colour"),
            )
            .on_click(cx.listener(|this, _, _, cx| this.change(None, cx)))
    }
}

impl Render for ColorPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (recent, mode) = cx.try_global::<Memory>().map_or_else(
            || (DEFAULT_RECENT.to_vec(), Mode::default()),
            |memory| (memory.recent.clone(), memory.mode),
        );
        v_flex()
            .w_full()
            .gap_4()
            .child(self.area(cx))
            .child(self.hue_row(cx))
            .child(self.fields(mode, cx))
            .child(self.recent(&recent, cx))
            .when(self.allow_none, |this| this.child(self.no_color(cx)))
    }
}

impl Swatch {
    pub(crate) fn new(id: impl Into<ElementId>, picker: &Entity<ColorPicker>) -> Self {
        Self {
            id: id.into(),
            picker: picker.clone(),
            size: px(SWATCH_SIZE),
        }
    }

    #[must_use]
    pub(crate) const fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Swatch {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let value = self.picker.read(cx).value();
        let size = self.size;
        div().flex_none().size(size).child(
            popover(self.id, &self.picker)
                .trigger(move |open, cx| swatch_face(value, open, size, cx)),
        )
    }
}

pub(crate) fn popover(id: impl Into<ElementId>, picker: &Entity<ColorPicker>) -> Dropdown {
    let picker = picker.clone();
    Dropdown::new(id)
        .menu_width(px(POPOVER_WIDTH))
        .content(move |_, _, _| div().p_3().child(picker.clone()).into_any_element())
}

pub(crate) fn hsla_of(color: Rgb) -> Hsla {
    Hsla::from(Rgba {
        r: f32::from(color.red) / CHANNEL_MAX,
        g: f32::from(color.green) / CHANNEL_MAX,
        b: f32::from(color.blue) / CHANNEL_MAX,
        a: 1.0,
    })
}

impl Render for AreaDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl Render for StripDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl Mode {
    const ALL: [Self; 2] = [Self::Rgb, Self::Hex];

    const fn label(self) -> &'static str {
        match self {
            Self::Rgb => "RGB",
            Self::Hex => "HEX",
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            recent: DEFAULT_RECENT.to_vec(),
            mode: Mode::default(),
        }
    }
}

impl Memory {
    fn remember(&mut self, color: Rgb) {
        self.recent.retain(|kept| *kept != color);
        self.recent.insert(0, color);
        self.recent.truncate(RECENT_LIMIT);
    }
}

impl Hsv {
    fn of(color: Rgb) -> Self {
        let max = color.red.max(color.green).max(color.blue);
        let min = color.red.min(color.green).min(color.blue);
        let spread = f32::from(max - min);
        let [red, green, blue] = [color.red, color.green, color.blue].map(f32::from);
        let sector = if max == min {
            0.0
        } else if max == color.red {
            (green - blue) / spread
        } else if max == color.green {
            2.0 + (blue - red) / spread
        } else {
            4.0 + (red - green) / spread
        };
        Self {
            hue: (sector / 6.0).rem_euclid(1.0),
            saturation: if max == 0 {
                0.0
            } else {
                spread / f32::from(max)
            },
            value: f32::from(max) / CHANNEL_MAX,
        }
    }

    fn adopt(self, color: Rgb) -> Self {
        let next = Self::of(color);
        let max = color.red.max(color.green).max(color.blue);
        let min = color.red.min(color.green).min(color.blue);
        if max == 0 {
            Self { value: 0.0, ..self }
        } else if max == min {
            Self {
                hue: self.hue,
                ..next
            }
        } else {
            next
        }
    }

    fn rgb(self) -> Rgb {
        let hue = self.hue.rem_euclid(1.0) * 6.0;
        let chroma = self.value * self.saturation;
        let rising = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
        let base = self.value - chroma;
        let (red, green, blue) = match sector(hue) {
            0 => (chroma, rising, 0.0),
            1 => (rising, chroma, 0.0),
            2 => (0.0, chroma, rising),
            3 => (0.0, rising, chroma),
            4 => (rising, 0.0, chroma),
            _ => (chroma, 0.0, rising),
        };
        Rgb {
            red: channel(red + base),
            green: channel(green + base),
            blue: channel(blue + base),
        }
    }
}

fn mode_face(mode: Mode, open: bool, cx: &App) -> AnyElement {
    let theme = cx.theme();
    h_flex()
        .h(px(FIELD_HEIGHT))
        .px_2()
        .gap_1()
        .justify_between()
        .rounded(theme.radius)
        .when(open, |this| this.bg(theme.secondary))
        .hover(|style| style.bg(theme.secondary))
        .child(div().text_sm().child(mode.label()))
        .child(
            Icon::new(if open {
                Catalog::ChevronUp
            } else {
                Catalog::ChevronDown
            })
            .xsmall()
            .text_color(theme.muted_foreground),
        )
        .into_any_element()
}

fn swatch_face(value: Option<Rgb>, open: bool, size: Pixels, cx: &App) -> AnyElement {
    let theme = cx.theme();
    div()
        .size(size)
        .p(px(2.0))
        .rounded_full()
        .border_2()
        .border_color(if open { theme.primary } else { theme.border })
        .child(value.map_or_else(
            || {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(Catalog::Ban)
                            .small()
                            .text_color(theme.muted_foreground),
                    )
            },
            |color| div().size_full().rounded_full().bg(hsla_of(color)),
        ))
        .into_any_element()
}

const fn color(red: u8, green: u8, blue: u8) -> Rgb {
    Rgb { red, green, blue }
}

fn fraction(offset: Pixels, length: Pixels) -> f32 {
    let length = f32::from(length);
    if length > 0.0 {
        (f32::from(offset) / length).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn sector(hue: f32) -> u8 {
    let floor = hue.floor().clamp(0.0, 5.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is floored and clamped to 0..=5"
    )]
    let sector = floor as u8;
    sector
}

fn channel(fraction: f32) -> u8 {
    let scaled = (fraction.clamp(0.0, 1.0) * CHANNEL_MAX).round();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is clamped to 0..=255 and rounded"
    )]
    let channel = scaled as u8;
    channel
}

fn hex_of(color: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
}

fn parse_hex(text: &str) -> Option<Rgb> {
    let digits = text.trim().trim_start_matches('#');
    if !digits.chars().all(|digit| digit.is_ascii_hexdigit()) {
        return None;
    }
    let expanded: String = match digits.len() {
        6 => digits.to_owned(),
        3 => digits.chars().flat_map(|digit| [digit, digit]).collect(),
        _ => return None,
    };
    let [_, red, green, blue] = u32::from_str_radix(&expanded, 16).ok()?.to_be_bytes();
    Some(Rgb { red, green, blue })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_colour_survives_the_hsv_round_trip() {
        let levels = (0..=255).step_by(15).chain([1, 128, 254]);
        for red in levels.clone() {
            for green in levels.clone() {
                for blue in levels.clone() {
                    let original = color(red, green, blue);
                    assert_eq!(Hsv::of(original).rgb(), original);
                }
            }
        }
    }

    #[test]
    fn greys_and_black_keep_the_hue_and_saturation_in_use() {
        let red = Hsv::of(color(255, 0, 0));
        let violet = Hsv { hue: 0.75, ..red };
        assert!((violet.adopt(color(128, 128, 128)).hue - 0.75).abs() < f32::EPSILON);
        let black = violet.adopt(color(0, 0, 0));
        assert!((black.hue - 0.75).abs() < f32::EPSILON);
        assert!((black.saturation - 1.0).abs() < f32::EPSILON);
        assert_eq!(black.rgb(), color(0, 0, 0));
    }

    #[test]
    fn hex_text_reads_six_and_three_digits() {
        assert_eq!(parse_hex("#00ff7f"), Some(color(0, 255, 127)));
        assert_eq!(parse_hex(" 0AF "), Some(color(0, 170, 255)));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("+12345"), None);
        assert_eq!(parse_hex("zzzzzz"), None);
        assert_eq!(hex_of(color(0, 255, 127)), "#00ff7f");
    }

    #[test]
    fn remembered_colours_move_to_the_front_without_duplicates() {
        let mut memory = Memory::default();
        memory.remember(color(1, 2, 3));
        memory.remember(color(0, 0, 255));
        assert_eq!(memory.recent.len(), RECENT_LIMIT);
        assert_eq!(memory.recent.first(), Some(&color(0, 0, 255)));
        assert_eq!(memory.recent.get(1), Some(&color(1, 2, 3)));
        assert_eq!(
            memory
                .recent
                .iter()
                .filter(|kept| **kept == color(0, 0, 255))
                .count(),
            1
        );
    }

    #[test]
    fn pointer_fractions_stay_within_the_control() {
        assert!((fraction(px(-5.0), px(100.0))).abs() < f32::EPSILON);
        assert!((fraction(px(50.0), px(100.0)) - 0.5).abs() < f32::EPSILON);
        assert!((fraction(px(150.0), px(100.0)) - 1.0).abs() < f32::EPSILON);
        assert!((fraction(px(1.0), px(0.0))).abs() < f32::EPSILON);
    }
}
