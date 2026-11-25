use std::rc::Rc;

use gpui_kit::{
    App, Div, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
    component::{ActiveTheme as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};

const CARD_HEIGHT: f32 = 40.0;
const CARD_PADDING: f32 = 12.0;
const CARD_RADIUS: f32 = 6.0;
const CARD_GAP: f32 = 8.0;
const LABEL_SIZE: f32 = 16.0;
const LABEL_LINE: f32 = 19.2;
const INDICATOR: f32 = 20.0;
const DOT: f32 = 8.0;

pub(super) fn choices<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: &[(T, &'static str)],
    selected: Option<T>,
    on_choose: impl Fn(&T, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let on_choose = Rc::new(on_choose);
    v_flex()
        .w_full()
        .gap(px(CARD_GAP))
        .children(options.iter().enumerate().map(|(index, (value, label))| {
            let value = *value;
            let chosen = selected == Some(value);
            let on_choose = on_choose.clone();
            let hover = theme.secondary;
            let group = SharedString::from(format!("{id}-{index}"));
            h_flex()
                .id((id, index))
                .group(group.clone())
                .w_full()
                .h(px(CARD_HEIGHT))
                .px(px(CARD_PADDING))
                .gap(px(CARD_GAP))
                .rounded(px(CARD_RADIUS))
                .bg(if chosen {
                    theme.secondary_hover
                } else {
                    theme.accent
                })
                .when(!chosen, |this| {
                    this.cursor_pointer().hover(move |style| style.bg(hover))
                })
                .child(indicator(chosen, group, cx))
                .child(
                    div()
                        .text_size(px(LABEL_SIZE))
                        .line_height(px(LABEL_LINE))
                        .text_color(theme.group_box_foreground)
                        .child(SharedString::from(*label)),
                )
                .on_click(move |_, window, cx| {
                    if !chosen {
                        on_choose(&value, window, cx);
                    }
                })
        }))
}

fn indicator(chosen: bool, group: SharedString, cx: &App) -> Div {
    let theme = cx.theme();
    let ring = theme.primary;
    div()
        .flex_none()
        .size(px(INDICATOR))
        .rounded_full()
        .border_1()
        .flex()
        .items_center()
        .justify_center()
        .map(|this| {
            if chosen {
                this.bg(theme.primary).border_color(theme.primary).child(
                    div()
                        .size(px(DOT))
                        .rounded_full()
                        .bg(theme.primary_foreground),
                )
            } else {
                this.bg(theme.accent)
                    .border_color(theme.switch)
                    .group_hover(group, move |style| style.border_color(ring))
            }
        })
}
