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
use crate::{keyboard::Dragged, theme};

const TILE: f32 = 44.0;
const TILE_SPACING: f32 = 8.0;
const TILE_RADIUS: f32 = 4.8;
const ICON: f32 = 38.0;
const DISABLED: f32 = 0.5;

impl GamepadPage {
    pub(super) fn palette(&self, enabled: bool, cx: &mut Context<Self>) -> Div {
        let controller = self.shown_controller();
        let ink = Ink::of(cx);
        let palette = theme::palette();
        let hover_border = cx.theme().muted_foreground;
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
                        .mr(px(TILE_SPACING))
                        .mb(px(TILE_SPACING))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(TILE_RADIUS))
                        .bg(palette.raised)
                        .border_1()
                        .border_color(palette.border_strong)
                        .child(
                            div()
                                .when(!enabled, |this| this.opacity(DISABLED))
                                .child(glyph::icon(control, controller, ink, px(ICON))),
                        )
                        .tooltip(move |window, cx| Tooltip::new(name).build(window, cx))
                        .map(|this| {
                            if enabled {
                                this.cursor_grab()
                                    .hover(|style| {
                                        style.bg(palette.raised_hover).border_color(hover_border)
                                    })
                                    .on_drag(
                                        Dragged::new(Carried::Control(control), name)
                                            .glyph(glyph::glyph(control, controller, ink, false)),
                                        |dragged, _, _, cx| cx.new(|_| dragged.clone()),
                                    )
                            } else {
                                this.cursor_not_allowed()
                            }
                        })
                });
        h_flex()
            .w_full()
            .flex_wrap()
            .on_drop(cx.listener(|this, _: &Dragged<Carried>, _, cx| {
                this.carried_from = None;
                cx.notify();
            }))
            .children(tiles)
    }
}
