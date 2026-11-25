use std::rc::Rc;

use gpui_kit::{
    App, Div, InteractiveElement as _, ParentElement as _, SharedString,
    StatefulInteractiveElement as _, Styled as _, Window,
    component::{ActiveTheme as _, h_flex, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};

const INDICATOR: f32 = 18.0;
const DOT: f32 = 6.0;

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
        .gap_2()
        .children(options.iter().enumerate().map(|(index, (value, label))| {
            let value = *value;
            let chosen = selected == Some(value);
            let on_choose = on_choose.clone();
            let hover = theme.secondary;
            h_flex()
                .id((id, index))
                .w_full()
                .h_10()
                .px_3()
                .gap_3()
                .rounded(theme.radius)
                .bg(if chosen {
                    theme.secondary_hover
                } else {
                    theme.group_box
                })
                .when(!chosen, |this| {
                    this.cursor_pointer().hover(move |style| style.bg(hover))
                })
                .child(indicator(chosen, cx))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.foreground)
                        .child(SharedString::from(*label)),
                )
                .on_click(move |_, window, cx| {
                    if !chosen {
                        on_choose(&value, window, cx);
                    }
                })
        }))
}

fn indicator(chosen: bool, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .flex_none()
        .size(px(INDICATOR))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(if chosen { theme.primary } else { theme.switch })
        .when(chosen, |this| {
            this.child(
                div()
                    .size(px(DOT))
                    .rounded_full()
                    .bg(theme.primary_foreground),
            )
        })
}
