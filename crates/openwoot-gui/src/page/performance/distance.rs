use std::mem;

use gpui_kit::{
    App, AppContext as _, Axis, Context, Div, Entity, EventEmitter, FocusHandle, Focusable as _,
    FontWeight, Hsla, InteractiveElement as _, IntoElement, ParentElement as _, Render,
    SharedString, StatefulInteractiveElement as _, Styled, Subscription, TextAlign, Window,
    assets::IconName,
    base::{Slider as Rail, SliderIndicator, SliderThumb, SliderTrack},
    component::{
        ActiveTheme as _, Icon, Sizable as _, h_flex,
        input::{Input, InputEvent, InputState},
        slider::{SliderEvent, SliderState},
        tooltip::Tooltip,
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px, relative,
};

use super::{
    ranges::{STEP_MM, Span},
    tuning::millimetres_text,
};

const TRACK: f32 = 4.0;
const THUMB: f32 = 16.0;
const RAIL: f32 = THUMB + 8.0;
const VERTICAL_LENGTH: f32 = 100.0;
const BOX_WIDTH: f32 = 52.0;
const BOX_HEIGHT: f32 = 32.0;
const MARKER: f32 = 16.0;
const TICK: f32 = 12.0;
const MIXED: &str = "mix";

pub(crate) struct DistanceField {
    axis: Axis,
    tags: bool,
    slider: Entity<SliderState>,
    input: Entity<InputState>,
    focus: FocusHandle,
    written: SharedString,
    shown: Shown,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Shown {
    pub(crate) millimetres: f32,
    pub(crate) span: Span,
    pub(crate) default: Option<f32>,
    pub(crate) mixed: bool,
    pub(crate) disabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Committed(pub(crate) f32);

impl EventEmitter<Committed> for DistanceField {}

impl DistanceField {
    pub(crate) fn horizontal(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(Axis::Horizontal, true, window, cx)
    }

    pub(crate) fn vertical(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(Axis::Vertical, false, window, cx)
    }

    pub(crate) fn plain(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(Axis::Horizontal, false, window, cx)
    }

    fn new(axis: Axis, tags: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let span = Span::actuation(0.0);
        let shown = Shown {
            millimetres: span.min,
            span,
            default: None,
            mixed: false,
            disabled: true,
        };
        let slider = cx.new(|_| {
            SliderState::new()
                .min(span.min)
                .max(span.max)
                .step(STEP_MM)
                .default_value(span.min)
        });
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(MIXED));
        let subscriptions = vec![
            cx.subscribe_in(
                &slider,
                window,
                |this, _, event: &SliderEvent, window, cx| match event {
                    SliderEvent::Change(value) => this.dragged(value.end(), window, cx),
                    SliderEvent::Release(value) => this.released(value.end(), window, cx),
                },
            ),
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                    this.typed(window, cx);
                }
            }),
        ];
        Self {
            axis,
            tags,
            slider,
            input,
            focus: cx.focus_handle(),
            written: SharedString::default(),
            shown,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn show(&mut self, shown: Shown, window: &mut Window, cx: &mut Context<Self>) {
        if self.shown == shown {
            return;
        }
        self.shown = shown;
        let position = self.position(shown.millimetres);
        self.slider.update(cx, |slider, cx| {
            let current = mem::replace(slider, SliderState::new());
            *slider = current.min(shown.span.min).max(shown.span.max);
            slider.set_value(position, window, cx);
        });
        let typed = self.input.read(cx).value() != self.written;
        self.input
            .update(cx, |input, cx| input.set_disabled(shown.disabled, cx));
        if !typed {
            self.write(self.text(), window, cx);
        }
        cx.notify();
    }

    fn position(&self, value: f32) -> f32 {
        match self.axis {
            Axis::Horizontal => value,
            Axis::Vertical => self.shown.span.min + self.shown.span.max - value,
        }
    }

    fn live(&self, cx: &App) -> f32 {
        self.position(self.slider.read(cx).value().end())
    }

    fn text(&self) -> SharedString {
        if self.shown.mixed {
            SharedString::default()
        } else {
            millimetres_text(self.shown.millimetres)
        }
    }

    fn reset_target(&self) -> Option<f32> {
        let default = self.shown.span.snap(self.shown.default?);
        let differs = (default - self.shown.millimetres).abs() > STEP_MM / 2.0;
        (!self.shown.disabled && (differs || self.shown.mixed)).then_some(default)
    }

    fn write(&mut self, text: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.written = text.clone();
        self.input
            .update(cx, |input, cx| input.set_value(text, window, cx));
    }

    fn dragged(&mut self, position: f32, window: &mut Window, cx: &mut Context<Self>) {
        let text = millimetres_text(self.shown.span.snap(self.position(position)));
        self.write(text, window, cx);
        cx.notify();
    }

    fn released(&mut self, position: f32, window: &mut Window, cx: &mut Context<Self>) {
        let millimetres = self.shown.span.snap(self.position(position));
        self.commit(millimetres, window, cx);
    }

    fn typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value();
        if text == self.written {
            return;
        }
        if let Some(millimetres) = parse(&text) {
            self.commit(self.shown.span.snap(millimetres), window, cx);
        } else {
            self.write(self.text(), window, cx);
        }
    }

    fn reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(default) = self.reset_target() {
            self.commit(default, window, cx);
        }
    }

    fn commit(&mut self, millimetres: f32, window: &mut Window, cx: &mut Context<Self>) {
        if self.shown.disabled {
            return;
        }
        self.shown.millimetres = millimetres;
        self.shown.mixed = false;
        let position = self.position(millimetres);
        self.slider
            .update(cx, |slider, cx| slider.set_value(position, window, cx));
        self.write(self.text(), window, cx);
        cx.emit(Committed(millimetres));
        cx.notify();
    }

    fn rail(&self, cx: &App) -> Div {
        let theme = cx.theme();
        let axis = self.axis;
        let disabled = self.shown.disabled;
        let live = self.live(cx);
        let fraction = self.shown.span.fraction(live);
        let fill = if disabled {
            theme.muted_foreground
        } else {
            theme.primary
        };
        let indicator = SliderIndicator::new(&self.slider)
            .relative()
            .rounded_full()
            .bg(theme.muted_foreground.opacity(0.6))
            .map(|this| match axis {
                Axis::Horizontal => this.w_full().h(px(TRACK)),
                Axis::Vertical => this.h_full().w(px(TRACK)),
            })
            .child(filled(axis, fraction, fill))
            .children(
                self.reset_target()
                    .map(|default| self.tick(default, live, cx)),
            )
            .child(self.thumb(fraction, fill, cx));
        let track = SliderTrack::new(&self.slider)
            .axis(axis)
            .disabled(disabled)
            .flex()
            .map(|this| match axis {
                Axis::Horizontal => this.items_center().h(px(RAIL)).w_full(),
                Axis::Vertical => this.justify_center().w(px(RAIL)).h_full(),
            })
            .child(indicator);
        let rail = Rail::new(&self.slider)
            .axis(axis)
            .disabled(disabled)
            .when(!disabled, |this| this.track_focus(&self.focus))
            .flex()
            .map(|this| match axis {
                Axis::Horizontal => this.w_full(),
                Axis::Vertical => this.h(px(VERTICAL_LENGTH)),
            })
            .child(track);
        div()
            .map(|this| match axis {
                Axis::Horizontal => this.w_full(),
                Axis::Vertical => this.h(px(VERTICAL_LENGTH)),
            })
            .when(!disabled, Styled::cursor_pointer)
            .child(rail)
    }

    fn thumb(&self, fraction: f32, ring: Hsla, cx: &App) -> SliderThumb {
        SliderThumb::new(&self.slider)
            .axis(self.axis)
            .disabled(self.shown.disabled)
            .absolute()
            .size(px(THUMB))
            .rounded_full()
            .bg(cx.theme().background)
            .border_2()
            .border_color(ring)
            .map(|this| match self.axis {
                Axis::Horizontal => this
                    .top(px((TRACK - THUMB) / 2.0))
                    .left(relative(fraction))
                    .ml(px(-THUMB / 2.0)),
                Axis::Vertical => this
                    .left(px((TRACK - THUMB) / 2.0))
                    .top(relative(fraction))
                    .mt(px(-THUMB / 2.0)),
            })
    }

    fn tick(&self, default: f32, live: f32, cx: &App) -> Div {
        let theme = cx.theme();
        let at = relative(self.shown.span.fraction(default));
        div()
            .absolute()
            .rounded_full()
            .bg(if live > default {
                theme.info
            } else {
                theme.secondary_active
            })
            .map(|this| match self.axis {
                Axis::Horizontal => this
                    .top_0()
                    .h_full()
                    .w(px(TICK))
                    .left(at)
                    .ml(px(-TICK / 2.0)),
                Axis::Vertical => this
                    .left_0()
                    .w_full()
                    .h(px(TICK))
                    .top(at)
                    .mt(px(-TICK / 2.0)),
            })
    }

    fn marker(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let hovered = theme.foreground;
        div()
            .relative()
            .w_full()
            .h(px(MARKER))
            .when_some(self.reset_target(), |this, default| {
                this.child(
                    div()
                        .id("reset-to-default")
                        .absolute()
                        .top_0()
                        .left(relative(self.shown.span.fraction(default)))
                        .ml(px(-MARKER / 2.0))
                        .size(px(MARKER))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_color(muted)
                        .hover(move |style| style.text_color(hovered))
                        .child(Icon::new(IconName::ChevronDown).small())
                        .tooltip(|window, cx| Tooltip::new("Reset to default").build(window, cx))
                        .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx))),
                )
            })
    }

    fn number(&self, window: &Window, cx: &App) -> Div {
        let theme = cx.theme();
        let focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        h_flex()
            .flex_none()
            .gap_2()
            .items_center()
            .child(
                div()
                    .id("distance-number")
                    .flex_none()
                    .w(px(BOX_WIDTH))
                    .h(px(BOX_HEIGHT))
                    .flex()
                    .items_center()
                    .rounded(theme.radius)
                    .bg(theme.sidebar)
                    .border_1()
                    .border_color(if focused { theme.primary } else { theme.border })
                    .when(self.shown.disabled, |this| this.opacity(0.5))
                    .child(
                        Input::new(&self.input)
                            .appearance(false)
                            .small()
                            .w_full()
                            .text_align(TextAlign::Center)
                            .disabled(self.shown.disabled),
                    )
                    .when(self.shown.mixed, |this| {
                        this.tooltip(|window, cx| {
                            Tooltip::new("The selected keys have different values.")
                                .build(window, cx)
                        })
                    }),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("mm"),
            )
    }

    fn tags(cx: &App) -> Div {
        let theme = cx.theme();
        let tag = |text: &'static str| {
            div()
                .px_1p5()
                .rounded(px(4.0))
                .bg(theme.secondary)
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.muted_foreground)
                .child(text)
        };
        h_flex()
            .w_full()
            .justify_between()
            .child(tag("HIGH"))
            .child(tag("LOW"))
    }
}

impl Render for DistanceField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let number = self.number(window, cx);
        let rail = self.rail(cx);
        match self.axis {
            Axis::Vertical => h_flex().gap_3().items_center().child(rail).child(number),
            Axis::Horizontal => h_flex()
                .w_full()
                .gap_4()
                .items_start()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(self.marker(cx))
                        .child(rail)
                        .when(self.tags, |this| this.child(Self::tags(cx))),
                )
                .child(
                    div()
                        .pt(px(MARKER + RAIL / 2.0 - BOX_HEIGHT / 2.0))
                        .child(number),
                ),
        }
    }
}

fn filled(axis: Axis, fraction: f32, color: Hsla) -> Div {
    div()
        .absolute()
        .top_0()
        .left_0()
        .rounded_full()
        .bg(color)
        .map(|this| match axis {
            Axis::Horizontal => this.h_full().w(relative(fraction)),
            Axis::Vertical => this.w_full().h(relative(fraction)),
        })
}

fn parse(text: &str) -> Option<f32> {
    text.trim()
        .replace(',', ".")
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_distances_accept_a_decimal_point_or_comma() {
        assert_eq!(parse("0.5"), Some(0.5));
        assert_eq!(parse(" 1,25 "), Some(1.25));
        assert_eq!(parse(""), None);
        assert_eq!(parse("mm"), None);
        assert_eq!(parse("inf"), None);
    }
}
