use gpui_kit::{
    AppContext as _, Context, Div, InteractiveElement as _, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _,
    component::{ActiveTheme as _, h_flex, tooltip::Tooltip},
    div,
    prelude::FluentBuilder as _,
    px,
};

use super::{
    Carried, GamepadPage, controls,
    glyph::{self, Ink},
};
use crate::keyboard::Dragged;

const TILE: f32 = 44.0;
const ICON: f32 = 36.0;
const DISABLED: f32 = 0.4;

impl GamepadPage {
    pub(super) fn palette(&self, enabled: bool, cx: &mut Context<Self>) -> Div {
        let controller = self.shown_controller();
        let ink = Ink::of(cx);
        let theme = cx.theme();
        let (surface, border, hover) = (theme.secondary, theme.border, theme.secondary_hover);
        let tiles =
            controller
                .palette()
                .iter()
                .copied()
                .enumerate()
                .map(move |(index, control)| {
                    let name = controls::name(control, controller);
                    div()
                        .id(("gamepad-tile", index))
                        .size(px(TILE))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(6.0))
                        .bg(surface)
                        .border_1()
                        .border_color(border)
                        .child(glyph::icon(control, controller, ink, px(ICON)))
                        .tooltip(move |window, cx| Tooltip::new(name).build(window, cx))
                        .when(enabled, |this| {
                            this.cursor_grab()
                                .hover(|style| style.border_color(hover))
                                .on_drag(
                                    Dragged::new(Carried::Control(control), name)
                                        .glyph(glyph::glyph(control, controller, ink, false)),
                                    |dragged, _, _, cx| cx.new(|_| dragged.clone()),
                                )
                        })
                        .when(!enabled, |this| this.opacity(DISABLED))
                });
        h_flex()
            .w_full()
            .flex_wrap()
            .gap_2()
            .on_drop(cx.listener(|this, _: &Dragged<Carried>, _, cx| {
                this.carried_from = None;
                cx.notify();
            }))
            .children(tiles)
    }
}
