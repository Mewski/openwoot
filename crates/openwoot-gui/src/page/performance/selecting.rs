use gpui_kit::{
    App, Div, Entity, ParentElement as _, SharedString, Styled as _, Window, component::h_flex,
};

use super::tuning::{Tuning, present};
use crate::{
    keyboard::{KeyFace, KeyboardView},
    panel::{ActionButton, Stage},
    store::Keyboard,
};

pub(crate) fn stage(view: KeyboardView, selected: usize, prompt: &str) -> Stage {
    Stage::new(view).caption(caption(selected, prompt), selected > 0)
}

fn caption(selected: usize, prompt: &str) -> SharedString {
    match selected {
        0 => SharedString::from(prompt.to_owned()),
        1 => "1 key selected".into(),
        count => format!("{count} keys selected").into(),
    }
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
            ActionButton::new("select-all-keys", "Select all keys")
                .disabled(selected >= total)
                .on_click(move |_, window, cx| select_all(window, cx)),
        )
        .child(
            ActionButton::new("discard-selection", "Discard selection")
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
    cx: &App,
) -> KeyboardView {
    let clicked = tuning.downgrade();
    let swept = tuning.downgrade();
    KeyboardView::new(id, keyboard, face)
        .selection(tuning.read(cx).selection())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captions_prompt_until_keys_are_selected() {
        assert_eq!(caption(0, "Select keys"), "Select keys");
        assert_eq!(caption(1, "Select keys"), "1 key selected");
        assert_eq!(caption(84, "Select keys"), "84 keys selected");
    }
}
