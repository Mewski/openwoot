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
use crate::{
    panel::{self, Tip},
    theme,
};

const TRACK: f32 = 4.0;
const THUMB: f32 = 16.0;
const RAIL: f32 = 14.0;
const VERTICAL_LENGTH: f32 = 100.0;
const LABELLED_LENGTH: f32 = 130.0;
const ROW_MAX_WIDTH: f32 = 480.0;
const ABOVE: f32 = 12.0;
const BELOW: f32 = 24.0;
const BOX_WIDTH: f32 = 48.0;
const BOX_RADIUS: f32 = 6.0;
const BOX_HEIGHT: f32 = 32.0;
const VALUE_TEXT: f32 = 12.8;
const VALUE_LINE: f32 = 19.2;
const VALUE_LEFT: f32 = 25.6;
const MARKER: f32 = 20.0;
const MARKER_OVERLAP: f32 = 3.0;
const RESET: f32 = 32.0;
const RESET_RADIUS: f32 = 6.0;
const RESET_ICON: f32 = 20.0;
const TAG_GAP: f32 = 8.0;
const TAG_HEIGHT: f32 = 20.0;
const DISABLED_RAIL_OPACITY: f32 = 0.6;
const DISABLED_BOX_OPACITY: f32 = 0.4;
const MIXED: &str = "mix";
const RESET_TO_DEFAULT: &str = "Reset to default";
const RESET_TO_GLOBAL: &str = "Reset to global";

pub(crate) struct DistanceField {
    axis: Axis,
    tags: bool,
    readout: Readout,
    slider: Entity<SliderState>,
    input: Entity<InputState>,
    focus: FocusHandle,
    written: SharedString,
    shown: Shown,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Readout {
    Input,
    Label,
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
        Self::new(Axis::Horizontal, true, Readout::Input, window, cx)
    }

    pub(crate) fn vertical(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(Axis::Vertical, false, Readout::Input, window, cx)
    }

    pub(crate) fn labelled(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(Axis::Vertical, false, Readout::Label, window, cx)
    }

    pub(crate) fn plain(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::new(Axis::Horizontal, false, Readout::Input, window, cx)
    }

    fn new(
        axis: Axis,
        tags: bool,
        readout: Readout,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
            readout,
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

    pub(crate) fn depth(&self, cx: &App) -> f32 {
        self.shown.span.fraction(self.live(cx))
    }

    fn length(&self) -> f32 {
        match self.readout {
            Readout::Input => VERTICAL_LENGTH,
            Readout::Label => LABELLED_LENGTH,
        }
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
            theme.sidebar_foreground
        } else {
            theme.primary
        };
        let ring = if disabled {
            theme.muted_foreground
        } else {
            theme.primary
        };
        let indicator = SliderIndicator::new(&self.slider)
            .relative()
            .rounded_full()
            .bg(theme.secondary_active)
            .map(|this| match axis {
                Axis::Horizontal => this.w_full().h(px(TRACK)),
                Axis::Vertical => this.h_full().w(px(TRACK)),
            })
            .child(filled(axis, fraction, fill))
            .children(
                self.reset_target()
                    .filter(|_| self.readout == Readout::Input)
                    .map(|default| self.tick(default, live, cx)),
            )
            .child(self.thumb(fraction, ring, cx));
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
                Axis::Vertical => this.h(px(self.length())),
            })
            .child(track);
        div()
            .map(|this| match axis {
                Axis::Horizontal => this.w_full(),
                Axis::Vertical => this.h(px(self.length())),
            })
            .when(disabled, |this| this.opacity(DISABLED_RAIL_OPACITY))
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
            .border_1()
            .border_color(ring)
            .shadow_sm()
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
                    .w(px(THUMB))
                    .left(at)
                    .ml(px(-THUMB / 2.0)),
                Axis::Vertical => this
                    .left_0()
                    .w_full()
                    .h(px(THUMB))
                    .top(at)
                    .mt(px(-THUMB / 2.0)),
            })
    }

    fn marker(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let theme = cx.theme();
        let muted = theme.muted_foreground;
        let hovered = theme.foreground;
        let default = self.reset_target()?;
        let marker = div()
            .id("reset-to-default")
            .absolute()
            .top(px(MARKER_OVERLAP - MARKER))
            .left(relative(self.shown.span.fraction(default)))
            .ml(px(-MARKER / 2.0))
            .size(px(MARKER))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_color(muted)
            .hover(move |style| style.text_color(hovered))
            .child(Icon::new(IconName::ChevronDown).size(px(MARKER)))
            .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx)));
        Some(panel::with_tip(
            marker,
            "reset-to-default-tip",
            Tip::Text(RESET_TO_DEFAULT.into()),
            window,
            cx,
        ))
    }

    fn reset_button(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let theme = cx.theme();
        let hover = theme.secondary;
        let foreground = theme.foreground;
        let default = self.reset_target()?;
        let centre = self.shown.span.fraction(default) * self.length();
        let button = div()
            .id("reset-to-global")
            .absolute()
            .left(px(-RESET))
            .top(px(centre - RESET / 2.0))
            .size(px(RESET))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(RESET_RADIUS))
            .bg(theme::palette().tint)
            .text_color(foreground)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .child(Icon::new(IconName::ChevronRight).size(px(RESET_ICON)))
            .on_click(cx.listener(|this, _, window, cx| this.reset(window, cx)));
        Some(panel::with_tip(
            button,
            "reset-to-global-tip",
            Tip::Text(RESET_TO_GLOBAL.into()),
            window,
            cx,
        ))
    }

    fn labelled_rail(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let live = self.live(cx);
        let centre = self.shown.span.fraction(live) * self.length();
        let value: SharedString = if self.shown.mixed {
            MIXED.into()
        } else {
            format!("{}mm", millimetres_text(self.shown.span.snap(live))).into()
        };
        div()
            .relative()
            .flex_none()
            .w(px(RAIL))
            .h(px(self.length()))
            .child(self.rail(cx))
            .child(
                div()
                    .absolute()
                    .left(px(VALUE_LEFT))
                    .top(px(centre - VALUE_LINE / 2.0))
                    .whitespace_nowrap()
                    .text_size(px(VALUE_TEXT))
                    .line_height(px(VALUE_LINE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::keycaps().legend)
                    .child(value),
            )
            .children(self.reset_button(window, cx))
    }

    fn number(&self, window: &Window, cx: &App) -> Div {
        let theme = cx.theme();
        let focused = self.input.read(cx).focus_handle(cx).is_focused(window);
        let disabled = self.shown.disabled;
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
                    .rounded(px(BOX_RADIUS))
                    .bg(theme::palette().input)
                    .border_1()
                    .border_color(if focused { theme.primary } else { theme.border })
                    .text_color(if disabled {
                        theme.secondary_active
                    } else {
                        theme.foreground
                    })
                    .when(disabled, |this| this.opacity(DISABLED_BOX_OPACITY))
                    .child(
                        Input::new(&self.input)
                            .appearance(false)
                            .xsmall()
                            .w_full()
                            .text_align(TextAlign::Center)
                            .disabled(disabled),
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
                    .text_size(px(VALUE_TEXT))
                    .line_height(px(VALUE_LINE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::keycaps().legend)
                    .child("mm"),
            )
    }

    fn tags(cx: &App) -> Div {
        let theme = cx.theme();
        let tag = |text: &'static str| {
            div()
                .h(px(TAG_HEIGHT))
                .px_1()
                .flex()
                .items_center()
                .rounded(px(BOX_RADIUS))
                .bg(theme.secondary)
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(theme.foreground)
                .child(text)
        };
        h_flex()
            .absolute()
            .top(px(RAIL + TAG_GAP))
            .left_0()
            .right_0()
            .justify_between()
            .child(tag("HIGH"))
            .child(tag("LOW"))
    }
}

impl Render for DistanceField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.readout == Readout::Label {
            return self.labelled_rail(window, cx);
        }
        let number = self.number(window, cx);
        let rail = self.rail(cx);
        match self.axis {
            Axis::Vertical => h_flex().gap_2().items_center().child(rail).child(number),
            Axis::Horizontal => h_flex()
                .w_full()
                .max_w(px(ROW_MAX_WIDTH))
                .pt(px(ABOVE))
                .pb(px(BELOW))
                .gap_2()
                .items_center()
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .child(rail)
                        .children(self.marker(window, cx))
                        .when(self.tags, |this| this.child(Self::tags(cx))),
                )
                .child(number),
        }
    }
}

pub(crate) fn distance_block(label: &'static str, field: &Entity<DistanceField>, cx: &App) -> Div {
    v_flex()
        .w_full()
        .gap_1()
        .child(panel::label(label, cx))
        .child(field.clone())
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
