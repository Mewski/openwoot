use gpui_kit::{
    App, AppContext as _, Context, Entity, IntoElement, ParentElement as _, Render, SharedString,
    Styled as _, Subscription, Window,
    component::{
        ActiveTheme as _, Sizable as _, WindowExt as _, h_flex,
        input::{Input, InputEvent, InputState},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::{
    device::{self, NAME_LEN},
    endpoints::SetDeviceName,
};

use crate::{
    panel::ActionButton,
    store::{Scope, Store},
    theme::palette,
};

const DIALOG_WIDTH: f32 = 684.0;
const GAP: f32 = 16.0;
const FOOTER_GAP: f32 = 8.0;
const HINT_SIZE: f32 = 14.0;
const COUNTER_SIZE: f32 = 12.0;

pub(super) struct Rename {
    store: Entity<Store>,
    input: Entity<InputState>,
    current: Option<SharedString>,
    _subscription: Subscription,
}

impl Rename {
    pub(super) fn open(store: &Entity<Store>, window: &mut Window, cx: &mut App) {
        let Some(keyboard) = store.read(cx).keyboard() else {
            return;
        };
        let product = keyboard.name.clone();
        let current = keyboard.device_name.clone();
        let store = store.clone();
        let rename = cx.new(|cx| Self::new(store, &product, current, window, cx));
        let focus = rename.read(cx).input.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let confirmed = rename.clone();
            dialog
                .title(SharedString::from(format!("Rename your {product}")))
                .w(px(DIALOG_WIDTH))
                .centered()
                .on_ok(move |_, _, cx| confirmed.update(cx, |rename, cx| rename.confirm(cx)))
                .child(rename.clone())
        });
        focus.update(cx, |input, cx| input.focus(window, cx));
    }

    fn new(
        store: Entity<Store>,
        product: &SharedString,
        current: Option<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(product.clone())
                .default_value(current.clone().unwrap_or_default())
                .validate(|text, _| fits(text))
        });
        let subscription = cx.subscribe(&input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            store,
            input,
            current,
            _subscription: subscription,
        }
    }

    fn typed(&self, cx: &App) -> SharedString {
        self.input.read(cx).value()
    }

    fn confirm(&self, cx: &mut Context<Self>) -> bool {
        let typed = self.typed(cx);
        let name = typed.trim();
        !name.is_empty() && self.commit(name, cx)
    }

    fn commit(&self, name: &str, cx: &mut Context<Self>) -> bool {
        let (Ok(before), Ok(after)) = (
            device::Name::try_from(self.current.as_deref().unwrap_or_default()),
            device::Name::try_from(name),
        ) else {
            return false;
        };
        if before != after {
            self.store.update(cx, |store, cx| {
                let renamed = store.change::<SetDeviceName>(&before, after, Scope::Runtime, cx);
                cx.spawn(async move |store, cx| {
                    if renamed.await.is_ok()
                        && let Ok(reload) = store.update(cx, |store, cx| store.reload(cx))
                    {
                        reload.await;
                    }
                })
                .detach();
            });
        }
        true
    }
}

impl Render for Rename {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let typed = self.typed(cx);
        let empty = typed.trim().is_empty();
        v_flex()
            .w_full()
            .gap(px(GAP))
            .child(
                div()
                    .text_size(px(HINT_SIZE))
                    .text_color(theme.sidebar_foreground)
                    .child("Enter a new name for your device."),
            )
            .child(
                Input::new(&self.input)
                    .large()
                    .bg(palette().input)
                    .cleanable(true)
                    .suffix(
                        div()
                            .text_size(px(COUNTER_SIZE))
                            .text_color(theme.sidebar_foreground)
                            .child(format!("{} / {NAME_LEN}", typed.len())),
                    ),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap(px(FOOTER_GAP))
                    .when(self.current.is_some(), |this| {
                        this.child(
                            ActionButton::new("revert-device-name", "Revert")
                                .ghost()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    if this.commit("", cx) {
                                        window.close_dialog(cx);
                                    }
                                })),
                        )
                    })
                    .child(div().flex_1())
                    .child(
                        ActionButton::new("cancel-rename", "Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        ActionButton::new("confirm-rename", "Confirm")
                            .primary()
                            .disabled(empty)
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.confirm(cx) {
                                    window.close_dialog(cx);
                                }
                            })),
                    ),
            )
    }
}

fn fits(text: &str) -> bool {
    text.len() <= NAME_LEN
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_fit_in_forty_bytes() {
        assert!(fits(&"a".repeat(NAME_LEN)));
        assert!(!fits(&"a".repeat(NAME_LEN + 1)));
        assert!(!fits(&"é".repeat(NAME_LEN / 2 + 1)));
    }
}
