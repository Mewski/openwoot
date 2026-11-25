use std::rc::Rc;

use gpui_kit::{
    AnyElement, App, BoxShadow, FocusHandle, Global, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, ParentElement as _, Pixels, Styled as _, Window, black,
    component::{ActiveTheme as _, IconName},
    deferred, div, point, px, relative,
};

use super::button::ActionButton;

const PADDING: f32 = 32.0;
const RADIUS: f32 = 8.0;
const SHADOW_Y: f32 = 4.0;
const SHADOW_BLUR: f32 = 8.0;
const SHADOW_ALPHA: f32 = 0.3;
const OVERLAY_ALPHA: f32 = 0.7;
const TOP_SHARE: f32 = 0.25;
const CLOSE_INSET: f32 = 12.0;
const CLOSE_GLYPH: f32 = 20.0;
const PRIORITY: usize = 1;

type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

#[derive(Clone)]
pub(crate) struct Modal {
    width: Pixels,
    content: Content,
}

#[derive(Default)]
struct Shown(Option<(Modal, FocusHandle)>);

impl Global for Shown {}

impl Modal {
    pub(crate) fn new(
        width: Pixels,
        content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        Self {
            width,
            content: Rc::new(content),
        }
    }

    pub(crate) fn open(self, window: &mut Window, cx: &mut App) {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        cx.set_global(Shown(Some((self, focus))));
        window.refresh();
    }

    pub(crate) fn close(cx: &mut App) {
        if cx
            .try_global::<Shown>()
            .is_some_and(|shown| shown.0.is_some())
        {
            cx.set_global(Shown::default());
        }
    }

    pub(crate) fn layer(window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        let (modal, focus) = cx.try_global::<Shown>()?.0.clone()?;
        let content = (modal.content)(window, cx);
        let theme = cx.theme();
        let surface = div()
            .id("modal")
            .relative()
            .w(modal.width)
            .mt(relative(TOP_SHARE))
            .p(px(PADDING))
            .rounded(px(RADIUS))
            .bg(theme.background)
            .shadow(vec![BoxShadow {
                color: black().opacity(SHADOW_ALPHA),
                offset: point(px(0.0), px(SHADOW_Y)),
                blur_radius: px(SHADOW_BLUR),
                spread_radius: px(0.0),
                inset: false,
            }])
            .track_focus(&focus)
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_key_down(|event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    dismiss(window, cx);
                }
            })
            .child(content)
            .child(
                ActionButton::icon_only("modal-close", IconName::Close)
                    .icon_size(px(CLOSE_GLYPH))
                    .text_color(theme.foreground)
                    .absolute()
                    .top(px(CLOSE_INSET))
                    .right(px(CLOSE_INSET))
                    .on_click(|_, window, cx| dismiss(window, cx)),
            );
        let overlay = div()
            .id("modal-overlay")
            .size_full()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(black().opacity(OVERLAY_ALPHA))
            .on_mouse_down(MouseButton::Left, |_, window, cx| dismiss(window, cx))
            .child(surface);
        Some(
            div()
                .absolute()
                .inset_0()
                .child(deferred(overlay).with_priority(PRIORITY))
                .into_any_element(),
        )
    }
}

fn dismiss(window: &mut Window, cx: &mut App) {
    Modal::close(cx);
    window.refresh();
}
