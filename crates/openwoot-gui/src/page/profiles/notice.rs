use gpui_kit::{
    AnyElement, App, FontWeight, IntoElement, ParentElement as _, Styled as _,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, h_flex, v_flex},
    div, px,
};

use crate::{panel::ActionButton, shell::OpenBackgroundService};

const SETUP_TOP: f32 = 32.0;
const SETUP_BOTTOM: f32 = 16.0;
const SETUP_GAP: f32 = 24.0;
const SETUP_WIDTH: f32 = 480.0;
const TEXT_GAP: f32 = 8.0;
const TITLE_TEXT: f32 = 20.0;
const TITLE_LINE: f32 = 24.0;
const BODY_TEXT: f32 = 16.0;
const BODY_LINE: f32 = 22.4;
const ALERT_PADDING: f32 = 12.0;
const ALERT_GAP: f32 = 12.0;
const ALERT_RADIUS: f32 = 8.0;
const ALERT_TINT: f32 = 0.16;
const ALERT_ICON: f32 = 20.0;
const ALERT_TEXT: f32 = 14.0;
const ALERT_LINE: f32 = 19.6;
const LINK: &str = "Go to Background Service";

pub(super) fn service_notice(linked: usize, cx: &App) -> AnyElement {
    if linked == 0 { setup(cx) } else { alert(cx) }
}

fn setup(cx: &App) -> AnyElement {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .items_center()
        .pt(px(SETUP_TOP))
        .pb(px(SETUP_BOTTOM))
        .gap(px(SETUP_GAP))
        .child(
            v_flex()
                .max_w(px(SETUP_WIDTH))
                .items_center()
                .gap(px(TEXT_GAP))
                .text_center()
                .child(
                    div()
                        .text_size(px(TITLE_TEXT))
                        .line_height(px(TITLE_LINE))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.group_box_foreground)
                        .child("Link profiles to your apps"),
                )
                .child(
                    div()
                        .text_size(px(BODY_TEXT))
                        .line_height(px(BODY_LINE))
                        .text_color(theme.sidebar_foreground)
                        .child(
                            "Turn on the Background Service to detect your active application and switch profiles automatically",
                        ),
                ),
        )
        .child(link_button("background-setup"))
        .into_any_element()
}

fn alert(cx: &App) -> AnyElement {
    let theme = cx.theme();
    h_flex()
        .w_full()
        .p(px(ALERT_PADDING))
        .gap(px(ALERT_GAP))
        .rounded(px(ALERT_RADIUS))
        .bg(theme.danger.opacity(ALERT_TINT))
        .child(
            Icon::new(Catalog::CircleAlert)
                .flex_none()
                .size(px(ALERT_ICON))
                .text_color(theme.danger),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .text_size(px(ALERT_TEXT))
                .line_height(px(ALERT_LINE))
                .child(
                    div()
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.foreground)
                        .child("Background Service is not running"),
                )
                .child(div().text_color(theme.group_box_foreground).child(
                    "Your linked profiles won't switch automatically until the Background Service is running.",
                )),
        )
        .child(div().flex_none().child(link_button("background-alert")))
        .into_any_element()
}

fn link_button(id: &'static str) -> ActionButton {
    ActionButton::new(id, LINK)
        .icon(Icon::new(Catalog::ArrowRight))
        .on_click(|_, window, cx| {
            window.dispatch_action(Box::new(OpenBackgroundService), cx);
        })
}
