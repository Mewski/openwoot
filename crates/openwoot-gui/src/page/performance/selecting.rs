use gpui_kit::{
    App, Div, Entity, FontWeight, ParentElement as _, SharedString, Styled as _, Window,
    component::{ActiveTheme as _, Disableable as _, Sizable as _, button::Button, h_flex, v_flex},
    div,
};

use super::tuning::{Tuning, present};
use crate::{
    keyboard::{KeyFace, KeyboardView},
    panel,
    store::Keyboard,
};

pub(crate) fn caption(selected: usize, prompt: &str, cx: &App) -> Div {
    let theme = cx.theme();
    let (text, color) = match selected {
        0 => (prompt.to_uppercase(), theme.muted_foreground),
        1 => ("1 KEY SELECTED".to_owned(), theme.primary),
        count => (format!("{count} KEYS SELECTED"), theme.primary),
    };
    h_flex().w_full().justify_center().child(
        div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(color)
            .child(SharedString::from(text)),
    )
}

pub(crate) fn stage(view: KeyboardView, selected: usize, prompt: &str, cx: &App) -> Div {
    v_flex()
        .w_full()
        .gap_4()
        .child(panel::stage(view))
        .child(caption(selected, prompt, cx))
}

pub(crate) fn actions(
    selected: usize,
    total: usize,
    select_all: impl Fn(&mut Window, &mut App) + 'static,
    discard: impl Fn(&mut Window, &mut App) + 'static,
) -> Div {
    h_flex()
        .gap_2()
        .child(
            Button::new("select-all-keys")
                .small()
                .label("Select all keys")
                .disabled(selected >= total)
                .on_click(move |_, window, cx| select_all(window, cx)),
        )
        .child(
            Button::new("discard-selection")
                .small()
                .label("Discard selection")
                .disabled(selected == 0)
                .on_click(move |_, window, cx| discard(window, cx)),
        )
}

pub(crate) fn tuning_actions(tuning: &Entity<Tuning>, keyboard: &Keyboard, cx: &App) -> Div {
    let selecting = tuning.downgrade();
    let discarding = tuning.downgrade();
    actions(
        tuning.read(cx).selection().len(),
        present(keyboard).count(),
        move |_, cx| {
            selecting.update(cx, Tuning::select_all).ok();
        },
        move |_, cx| {
            discarding.update(cx, Tuning::clear_selection).ok();
        },
    )
}

pub(crate) fn tuning_keyboard(
    id: &'static str,
    keyboard: &Keyboard,
    tuning: &Entity<Tuning>,
    face: impl Fn(usize) -> KeyFace,
    window: &Window,
    cx: &App,
) -> KeyboardView {
    let clicked = tuning.downgrade();
    let swept = tuning.downgrade();
    KeyboardView::new(id, keyboard, face)
        .selection(tuning.read(cx).selection())
        .fit_width(panel::content_width(window))
        .on_key_click(move |key, _, _, cx| {
            clicked
                .update(cx, |tuning, cx| tuning.toggle_key(key, cx))
                .ok();
        })
        .on_key_sweep(move |key, _, cx| {
            swept
                .update(cx, |tuning, cx| tuning.sweep_key(key, cx))
                .ok();
        })
}
