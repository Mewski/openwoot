use std::{cell::Cell, mem, rc::Rc};

use gpui_kit::{
    AppContext as _, Bounds, Context, Div, DragMoveEvent, Empty, Entity, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, ParentElement as _, Pixels, Point, Render,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window,
    assets::IconName as Catalog,
    canvas,
    component::{
        ActiveTheme as _, Selectable as _, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        input::{InputEvent, InputState, NumberInput},
        slider::{Slider, SliderEvent, SliderState},
    },
    div, hsla, linear_color_stop, linear_gradient,
    prelude::FluentBuilder as _,
    px, relative,
};
use openwoot_protocol::lighting::Rgb;

use super::{LightingPage, WHITE, color::Hsv, hsla_of};
use crate::panel;

const HUE_MAX: f32 = 360.0;
const HUE_STEPS: u16 = 48;
const AREA_HEIGHT: f32 = 168.0;
const KNOB_SIZE: f32 = 14.0;
const SWATCH_SIZE: f32 = 28.0;
const RECENT_LIMIT: usize = 12;
const CHANNELS: [&str; 3] = ["R", "G", "B"];

pub(super) struct Brush {
    color: Hsv,
    hue: Entity<SliderState>,
    channels: [Entity<InputState>; 3],
    recent: Vec<Rgb>,
    area: Rc<Cell<Bounds<Pixels>>>,
    dragging: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Hue,
    Channels,
    Other,
}

struct AreaDrag;

impl Render for AreaDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Empty
    }
}

impl Brush {
    pub(super) fn new(window: &mut Window, cx: &mut Context<LightingPage>) -> Self {
        let color = Hsv::of(WHITE);
        let hue = cx.new(|_| SliderState::new().min(0.0).max(HUE_MAX).step(1.0));
        let channels = [WHITE.red, WHITE.green, WHITE.blue].map(|channel| {
            cx.new(|cx| {
                let mut input = InputState::new(window, cx);
                input.set_value(channel.to_string(), window, cx);
                input
            })
        });
        Self {
            color,
            hue,
            channels,
            recent: Vec::new(),
            area: Rc::default(),
            dragging: false,
        }
    }

    pub(super) fn rgb(&self) -> Rgb {
        self.color.rgb()
    }

    pub(super) fn subscriptions(
        &self,
        window: &mut Window,
        cx: &mut Context<LightingPage>,
    ) -> Vec<Subscription> {
        let mut subscriptions = vec![cx.subscribe_in(
            &self.hue,
            window,
            |this, _, event: &SliderEvent, window, cx| match event {
                SliderEvent::Change(value) => {
                    let color = Hsv {
                        hue: value.end() / HUE_MAX,
                        ..this.brush.color
                    };
                    this.set_brush(color, Source::Hue, window, cx);
                }
                SliderEvent::Release(_) => this.remember_brush(cx),
            },
        )];
        for channel in &self.channels {
            subscriptions.push(cx.subscribe_in(
                channel,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => this.type_channels(window, cx),
                    InputEvent::PressEnter { .. } | InputEvent::Blur => this.remember_brush(cx),
                    InputEvent::Focus => {}
                },
            ));
        }
        subscriptions
    }

    fn remember(&mut self, color: Rgb) {
        self.recent.retain(|kept| *kept != color);
        self.recent.insert(0, color);
        self.recent.truncate(RECENT_LIMIT);
    }
}

impl LightingPage {
    pub(super) fn colors_card(&self, cx: &Context<Self>) -> Option<Div> {
        self.maps.as_ref()?;
        Some(
            panel::section("Colours", None, cx)
                .child(self.area(cx))
                .child(self.hue_row(cx))
                .child(self.channel_row(cx))
                .children(self.recent_row(cx)),
        )
    }

    pub(super) fn pick_brush(&mut self, color: Rgb, window: &mut Window, cx: &mut Context<Self>) {
        let color = self.steady(color);
        self.set_brush(color, Source::Other, window, cx);
        self.brush.remember(color.rgb());
    }

    fn area(&self, cx: &Context<Self>) -> impl IntoElement {
        let radius = cx.theme().radius;
        let color = self.brush.color;
        let bounds = Rc::clone(&self.brush.area);
        let white = hsla(0.0, 0.0, 1.0, 1.0);
        let black = hsla(0.0, 0.0, 0.0, 1.0);
        let layer = || {
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .rounded(radius)
        };
        div()
            .id("brush-area")
            .relative()
            .w_full()
            .h(px(AREA_HEIGHT))
            .rounded(radius)
            .cursor_crosshair()
            .bg(hsla(color.hue, 1.0, 0.5, 1.0))
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
            .child(
                div()
                    .absolute()
                    .left(relative(color.saturation))
                    .top(relative(1.0 - color.value))
                    .ml(px(-KNOB_SIZE / 2.0))
                    .mt(px(-KNOB_SIZE / 2.0))
                    .size(px(KNOB_SIZE))
                    .rounded_full()
                    .border_2()
                    .border_color(white)
                    .bg(hsla_of(color.rgb())),
            )
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
                    this.brush.dragging = true;
                    let bounds = this.brush.area.get();
                    this.drag_area(event.position, bounds, window, cx);
                }),
            )
            .on_drag(AreaDrag, |_, _, _, cx| cx.new(|_| AreaDrag))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<AreaDrag>, window, cx| {
                    this.drag_area(event.event.position, event.bounds, window, cx);
                }),
            )
            .on_drop(cx.listener(|this, _: &AreaDrag, _, cx| this.release_area(cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.release_area(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.release_area(cx)),
            )
    }

    fn hue_row(&self, cx: &Context<Self>) -> Div {
        let track = h_flex()
            .absolute()
            .left_0()
            .right_0()
            .h_2p5()
            .rounded_full()
            .overflow_hidden()
            .children((0..HUE_STEPS).map(|step| {
                let hue = f32::from(step) / f32::from(HUE_STEPS - 1);
                div().flex_1().h_full().bg(hsla(hue, 1.0, 0.5, 1.0))
            }));
        h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .child(
                Button::new("pick-color")
                    .icon(Catalog::Pipette)
                    .ghost()
                    .small()
                    .selected(self.picking)
                    .tooltip("Pick a colour from a key")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.picking = !this.picking;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_1()
                    .items_center()
                    .h_6()
                    .child(track)
                    .child(
                        Slider::new(&self.brush.hue)
                            .flex_1()
                            .bg(cx.theme().transparent),
                    ),
            )
    }

    fn channel_row(&self, cx: &Context<Self>) -> Div {
        let muted = cx.theme().muted_foreground;
        h_flex()
            .w_full()
            .gap_2()
            .children(
                self.brush
                    .channels
                    .iter()
                    .zip(CHANNELS)
                    .map(|(state, name)| {
                        div().flex_1().min_w_0().child(
                            NumberInput::new(state)
                                .small()
                                .stepper(false)
                                .prefix(div().text_xs().text_color(muted).child(name)),
                        )
                    }),
            )
    }

    fn recent_row(&self, cx: &Context<Self>) -> Option<Div> {
        if self.brush.recent.is_empty() {
            return None;
        }
        let theme = cx.theme();
        let current = self.brush.rgb();
        let swatches = self.brush.recent.iter().enumerate().map(|(index, color)| {
            let color = *color;
            div()
                .id(SharedString::from(format!("recent-colour-{index}")))
                .size(px(SWATCH_SIZE))
                .rounded_full()
                .bg(hsla_of(color))
                .cursor_pointer()
                .map(|this| {
                    if color == current {
                        this.border_2().border_color(theme.primary)
                    } else {
                        this.border_1().border_color(theme.border)
                    }
                })
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.pick_brush(color, window, cx);
                }))
        });
        Some(panel::field(
            "Recently used",
            h_flex().flex_wrap().gap_2().children(swatches),
            cx,
        ))
    }

    fn drag_area(
        &mut self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let across = f32::from(position.x - bounds.origin.x) / width;
        let down = f32::from(position.y - bounds.origin.y) / height;
        let color = Hsv {
            hue: self.brush.color.hue,
            saturation: across.clamp(0.0, 1.0),
            value: 1.0 - down.clamp(0.0, 1.0),
        };
        self.set_brush(color, Source::Other, window, cx);
    }

    fn release_area(&mut self, cx: &mut Context<Self>) {
        if mem::take(&mut self.brush.dragging) {
            self.remember_brush(cx);
        }
    }

    fn remember_brush(&mut self, cx: &mut Context<Self>) {
        let color = self.brush.rgb();
        self.brush.remember(color);
        cx.notify();
    }

    fn type_channels(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let [red, green, blue] = self
            .brush
            .channels
            .each_ref()
            .map(|channel| channel.read(cx).value().trim().parse::<u8>().ok());
        let (Some(red), Some(green), Some(blue)) = (red, green, blue) else {
            return;
        };
        let color = self.steady(Rgb { red, green, blue });
        self.set_brush(color, Source::Channels, window, cx);
    }

    fn steady(&self, color: Rgb) -> Hsv {
        let mut next = Hsv::of(color);
        if next.saturation <= 0.0 || next.value <= 0.0 {
            next.hue = self.brush.color.hue;
        }
        next
    }

    fn set_brush(
        &mut self,
        color: Hsv,
        source: Source,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.brush.color = color;
        if source != Source::Hue {
            let hue = color.hue * HUE_MAX;
            self.brush
                .hue
                .update(cx, |slider, cx| slider.set_value(hue, window, cx));
        }
        if source != Source::Channels {
            let rgb = color.rgb();
            for (channel, value) in self
                .brush
                .channels
                .iter()
                .zip([rgb.red, rgb.green, rgb.blue])
            {
                channel.update(cx, |input, cx| {
                    input.set_value(value.to_string(), window, cx);
                });
            }
        }
        self.paint_selection(window, cx);
        cx.notify();
    }
}
