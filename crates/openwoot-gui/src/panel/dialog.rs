use std::rc::Rc;

use gpui_kit::{
    AnyElement, App, BoxShadow, Div, FontWeight, IntoElement, ParentElement as _, SharedString,
    Styled as _, Window,
    component::{ActiveTheme as _, IconName, WindowExt as _, h_flex, v_flex},
    div, hsla, point,
    prelude::FluentBuilder as _,
    px,
};

use super::button::ActionButton;
use crate::theme;

const WIDTH: f32 = 448.0;
const RADIUS: f32 = 6.0;
const HEADER_RADIUS: f32 = 8.0;
const HEADER_PADDING_X: f32 = 16.0;
const HEADER_PADDING_Y: f32 = 12.0;
const TITLE_TEXT: f32 = 20.0;
const TITLE_LINE: f32 = 24.0;
const COMPACT_TITLE_TEXT: f32 = 16.0;
const COMPACT_TITLE_LINE: f32 = 19.2;
const CLOSE_TOP: f32 = 8.0;
const CLOSE_RIGHT: f32 = 12.0;
const CLOSE_GLYPH: f32 = 20.0;
const BODY_PADDING: f32 = 16.0;
const BODY_TEXT: f32 = 16.0;
const BODY_LINE: f32 = 22.4;
const ALERT_BODY_LINE: f32 = 24.0;
const COMPACT_BODY_TEXT: f32 = 12.8;
const COMPACT_BODY_LINE: f32 = 19.2;
const FOOTER_GAP: f32 = 8.0;
const ALERT_FOOTER_GAP: f32 = 12.0;

type Confirmed = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
pub(crate) struct Confirmation {
    title: SharedString,
    text: SharedString,
    action: SharedString,
    tone: Tone,
    look: Look,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Primary,
    Danger,
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    Modal,
    Alert,
    Compact,
}

impl Confirmation {
    pub(crate) fn new(
        title: impl Into<SharedString>,
        text: impl Into<SharedString>,
        action: impl Into<SharedString>,
    ) -> Self {
        Self {
            title: title.into(),
            text: text.into(),
            action: action.into(),
            tone: Tone::Primary,
            look: Look::Modal,
        }
    }

    pub(crate) fn danger(mut self) -> Self {
        self.tone = Tone::Danger;
        self
    }

    pub(crate) fn neutral(mut self) -> Self {
        self.tone = Tone::Neutral;
        self
    }

    pub(crate) fn alert(mut self) -> Self {
        self.look = Look::Alert;
        self
    }

    pub(crate) fn compact(mut self) -> Self {
        self.look = Look::Compact;
        self
    }
}

pub(crate) fn confirm(
    confirmation: Confirmation,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    window: &mut Window,
    cx: &mut App,
) {
    let on_confirm: Confirmed = Rc::new(on_confirm);
    window.open_dialog(cx, move |dialog, _, cx| {
        let surface = if confirmation.look == Look::Compact {
            cx.theme().sidebar
        } else {
            cx.theme().tab_active
        };
        dialog
            .w(px(WIDTH))
            .centered()
            .close_button(false)
            .p_0()
            .gap_0()
            .border_0()
            .rounded(px(RADIUS))
            .bg(surface)
            .shadow(shadow())
            .child(
                v_flex()
                    .relative()
                    .w_full()
                    .child(header(&confirmation, cx))
                    .child(body(&confirmation, cx))
                    .child(footer(&confirmation, &on_confirm))
                    .when(confirmation.look == Look::Modal, |this| {
                        this.child(close_button(cx))
                    }),
            )
    });
}

fn header(confirmation: &Confirmation, cx: &App) -> Div {
    let theme = cx.theme();
    let (text, line, color) = match confirmation.look {
        Look::Modal | Look::Alert => (TITLE_TEXT, TITLE_LINE, theme.foreground),
        Look::Compact => (
            COMPACT_TITLE_TEXT,
            COMPACT_TITLE_LINE,
            theme::palette().heading,
        ),
    };
    div()
        .w_full()
        .px(px(HEADER_PADDING_X))
        .py(px(HEADER_PADDING_Y))
        .rounded_t(px(HEADER_RADIUS))
        .bg(theme::palette().overlay_hover)
        .border_b_1()
        .border_color(theme.border)
        .text_size(px(text))
        .line_height(px(line))
        .font_weight(FontWeight::BOLD)
        .text_color(color)
        .child(confirmation.title.clone())
}

fn body(confirmation: &Confirmation, cx: &App) -> Div {
    let theme = cx.theme();
    let text = div()
        .w_full()
        .pt(px(BODY_PADDING))
        .px(px(BODY_PADDING))
        .child(confirmation.text.clone());
    match confirmation.look {
        Look::Modal => text
            .text_size(px(BODY_TEXT))
            .line_height(px(BODY_LINE))
            .text_color(theme.group_box_foreground),
        Look::Alert => text
            .text_size(px(BODY_TEXT))
            .line_height(px(ALERT_BODY_LINE))
            .text_color(theme.foreground),
        Look::Compact => text
            .text_size(px(COMPACT_BODY_TEXT))
            .line_height(px(COMPACT_BODY_LINE))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme::keycaps().legend),
    }
}

fn footer(confirmation: &Confirmation, on_confirm: &Confirmed) -> Div {
    let on_confirm = on_confirm.clone();
    let row = h_flex().w_full().p(px(BODY_PADDING));
    let row = match confirmation.look {
        Look::Modal => row.gap(px(FOOTER_GAP)).justify_end(),
        Look::Alert => row.gap(px(ALERT_FOOTER_GAP)).justify_end(),
        Look::Compact => row.justify_between(),
    };
    row.child(
        ActionButton::new("dialog-cancel", "Cancel")
            .on_click(|_, window, cx| window.close_dialog(cx)),
    )
    .child(action_button(confirmation).on_click(move |_, window, cx| {
        window.close_dialog(cx);
        on_confirm(window, cx);
    }))
}

fn close_button(cx: &App) -> AnyElement {
    ActionButton::icon_only("dialog-close", IconName::Close)
        .icon_size(px(CLOSE_GLYPH))
        .text_color(cx.theme().foreground)
        .absolute()
        .top(px(CLOSE_TOP))
        .right(px(CLOSE_RIGHT))
        .on_click(|_, window, cx| window.close_dialog(cx))
        .into_any_element()
}

fn shadow() -> Vec<BoxShadow> {
    [
        (0.1, 0.0, 0.0, 1.0),
        (0.2, 5.0, 10.0, 0.0),
        (0.4, 15.0, 40.0, 0.0),
    ]
    .into_iter()
    .map(|(alpha, y, blur, spread)| BoxShadow {
        color: hsla(0.0, 0.0, 0.0, alpha),
        offset: point(px(0.0), px(y)),
        blur_radius: px(blur),
        spread_radius: px(spread),
        inset: false,
    })
    .collect()
}

fn action_button(confirmation: &Confirmation) -> ActionButton {
    let button = ActionButton::new("dialog-confirm", confirmation.action.clone());
    match confirmation.tone {
        Tone::Primary => button.primary(),
        Tone::Danger => button.danger(),
        Tone::Neutral => button,
    }
}
