use gpui_kit::{
    App, ClickEvent, Div, FontWeight, ParentElement as _, Styled as _, Window,
    component::{ActiveTheme as _, h_flex, v_flex},
    div, px,
};

use super::section;
use crate::{page::settings::emphasised, panel::ActionButton};

const STEPS: [(&str, &str); 2] = [
    (
        "Hold the keys and plug in",
        "Disconnect your keyboard, hold the top-right and bottom-right keys, then replug it.",
    ),
    (
        "Verify key colours",
        "Check if the top-right and bottom-right keys are glowing blue.",
    ),
];
const KEYS: [&str; 2] = ["top-right", "bottom-right"];
const STEP_GAP: f32 = 8.0;
const STEP_RADIUS: f32 = 6.0;
const STEP_PADDING_X: f32 = 24.0;
const STEP_PADDING_Y: f32 = 16.0;
const STEP_INNER_GAP: f32 = 12.0;
const BADGE: f32 = 20.0;
const BADGE_SIZE: f32 = 14.0;
const HEADER_GAP: f32 = 8.0;
const STEP_TITLE_SIZE: f32 = 16.0;
const STEP_TITLE_LINE: f32 = 20.0;
const STEP_TEXT_SIZE: f32 = 16.0;
const STEP_TEXT_LINE: f32 = 22.4;
const ACTION_GAP: f32 = 24.0;

pub(super) fn restore_section(
    busy: bool,
    on_restore: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    section(
        "Troubleshoot",
        "Follow the steps below to restore and reset your keyboard.",
        cx,
    )
    .child(
        h_flex()
            .w_full()
            .items_stretch()
            .gap(px(STEP_GAP))
            .children(
                STEPS
                    .iter()
                    .enumerate()
                    .map(|(index, (title, text))| step(index + 1, title, text, cx)),
            ),
    )
    .child(
        h_flex().w_full().justify_end().mt(px(ACTION_GAP)).child(
            ActionButton::new("restore-keyboard", "Restore and reset device")
                .primary()
                .large()
                .disabled(busy)
                .on_click(on_restore),
        ),
    )
}

fn step(number: usize, title: &'static str, text: &'static str, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .flex_1()
        .min_w_0()
        .gap(px(STEP_INNER_GAP))
        .px(px(STEP_PADDING_X))
        .py(px(STEP_PADDING_Y))
        .rounded(px(STEP_RADIUS))
        .bg(theme.tab_active)
        .child(
            h_flex()
                .w_full()
                .items_start()
                .gap(px(HEADER_GAP))
                .child(
                    div()
                        .flex_none()
                        .size(px(BADGE))
                        .rounded_full()
                        .bg(theme.primary)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(BADGE_SIZE))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.primary_foreground)
                        .child(number.to_string()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(STEP_TITLE_SIZE))
                        .line_height(px(STEP_TITLE_LINE))
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.foreground)
                        .child(title),
                ),
        )
        .child(
            div()
                .text_size(px(STEP_TEXT_SIZE))
                .line_height(px(STEP_TEXT_LINE))
                .text_color(theme.group_box_foreground)
                .child(emphasised(text, &KEYS)),
        )
}
