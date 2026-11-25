use gpui_kit::{
    AnyElement, App, Bounds, BoxShadow, Div, ElementId, FontWeight, IntoElement,
    ParentElement as _, Pixels, SharedString, Size, Stateful, StatefulInteractiveElement as _,
    Styled as _, Window, anchored, canvas,
    component::{ActiveTheme as _, h_flex, v_flex},
    deferred, div, hsla, point,
    prelude::FluentBuilder as _,
    px,
};

use super::chip::{KBD_GAP, kbd};
use crate::theme;

const GAP: f32 = 8.0;
const WINDOW_MARGIN: f32 = 8.0;
const PRIORITY: usize = usize::MAX;
const RADIUS: f32 = 6.0;
const TEXT: f32 = 14.0;
const LINE: f32 = 21.0;
const PADDING_X: f32 = 12.0;
const TEXT_MAX_WIDTH: f32 = 320.0;
const PADDING_Y: f32 = 8.0;
const HELP_WIDTH: f32 = 440.0;
const HELP_PADDING: f32 = 16.0;
const HELP_GAP: f32 = 8.0;
const HELP_TEXT: f32 = 16.0;
const HELP_TITLE_LINE: f32 = 19.2;
const HELP_BODY_LINE: f32 = 22.4;
const KEYS_TEXT: f32 = 11.2;

#[derive(Clone)]
pub(crate) enum Tip {
    Text(SharedString),
    Help {
        title: SharedString,
        text: SharedString,
    },
    Keys {
        keys: Vec<SharedString>,
        text: SharedString,
    },
}

#[derive(Default)]
struct TipState {
    shown: bool,
    bounds: Bounds<Pixels>,
    size: Size<Pixels>,
}

pub(crate) fn with_tip(
    element: Stateful<Div>,
    key: impl Into<ElementId>,
    tip: Tip,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    tip_layer(element, key.into(), tip, false, window, cx)
}

pub(crate) fn with_tip_right(
    element: Stateful<Div>,
    key: impl Into<ElementId>,
    tip: Tip,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    tip_layer(element, key.into(), tip, true, window, cx)
}

fn tip_layer(
    element: Stateful<Div>,
    key: ElementId,
    tip: Tip,
    right: bool,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let state = window.use_keyed_state(key, cx, |_, _| TipState::default());
    let hovered = state.clone();
    let measured = state.clone();
    let sized = state.clone();
    let (shown, bounds, size) = {
        let current = state.read(cx);
        (current.shown, current.bounds, current.size)
    };
    let room_above = bounds.top() - px(GAP) - size.height >= px(WINDOW_MARGIN);
    let (anchor, corner, offset) = match (right, room_above) {
        (true, _) => (
            gpui_kit::Anchor::LeftCenter,
            gpui_kit::Anchor::RightCenter,
            point(px(GAP), px(0.0)),
        ),
        (false, true) => (
            gpui_kit::Anchor::BottomCenter,
            gpui_kit::Anchor::TopCenter,
            point(px(0.0), px(-GAP)),
        ),
        (false, false) => (
            gpui_kit::Anchor::TopCenter,
            gpui_kit::Anchor::BottomCenter,
            point(px(0.0), px(GAP)),
        ),
    };
    element
        .on_hover(move |hovering, _, cx| {
            hovered.update(cx, |state, cx| {
                state.shown = *hovering;
                cx.notify();
            });
        })
        .child(
            canvas(
                move |bounds, _, cx| measured.update(cx, |state, _| state.bounds = bounds),
                |_, (), _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .when(shown, |this| {
            this.child(
                deferred(
                    anchored()
                        .anchor(anchor)
                        .attach(bounds, corner)
                        .offset(offset)
                        .snap_to_window_with_margin(px(WINDOW_MARGIN))
                        .child(
                            div().relative().child(tip_surface(tip, cx)).child(
                                canvas(
                                    move |bounds, _, cx| {
                                        sized.update(cx, |state, cx| {
                                            if state.size != bounds.size {
                                                state.size = bounds.size;
                                                cx.notify();
                                            }
                                        });
                                    },
                                    |_, (), _, _| {},
                                )
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full(),
                            ),
                        ),
                )
                .with_priority(PRIORITY),
            )
        })
}

pub(crate) fn tip_surface(tip: Tip, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let base = v_flex()
        .rounded(px(RADIUS))
        .bg(theme::palette().tooltip)
        .border_1()
        .border_color(theme.border)
        .shadow(shadow())
        .text_color(theme.foreground);
    match tip {
        Tip::Text(text) => base
            .px(px(PADDING_X))
            .py(px(PADDING_Y))
            .text_size(px(TEXT))
            .line_height(px(LINE))
            .font_weight(FontWeight::SEMIBOLD)
            .max_w(px(TEXT_MAX_WIDTH))
            .child(text)
            .into_any_element(),
        Tip::Keys { keys, text } => {
            let legend = theme::keycaps().legend;
            base.px(px(PADDING_X))
                .py(px(PADDING_Y))
                .text_size(px(TEXT))
                .line_height(px(LINE))
                .font_weight(FontWeight::SEMIBOLD)
                .child(
                    h_flex()
                        .gap(px(KBD_GAP))
                        .children(
                            keys.into_iter()
                                .map(|key| kbd(key, legend, cx).text_size(px(KEYS_TEXT))),
                        )
                        .child(text),
                )
                .into_any_element()
        }
        Tip::Help { title, text } => base
            .w(px(HELP_WIDTH))
            .p(px(HELP_PADDING))
            .gap(px(HELP_GAP))
            .text_size(px(HELP_TEXT))
            .child(
                div()
                    .line_height(px(HELP_TITLE_LINE))
                    .font_weight(FontWeight::BOLD)
                    .child(title),
            )
            .child(div().line_height(px(HELP_BODY_LINE)).child(text))
            .into_any_element(),
    }
}

fn shadow() -> Vec<BoxShadow> {
    [(0.1, 4.0, 6.0), (0.06, 2.0, 4.0)]
        .into_iter()
        .map(|(alpha, y, blur)| BoxShadow {
            color: hsla(0.0, 0.0, 0.0, alpha),
            offset: point(px(0.0), px(y)),
            blur_radius: px(blur),
            spread_radius: px(-1.0),
            inset: false,
        })
        .collect()
}
