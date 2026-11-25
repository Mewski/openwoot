use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, h_flex, tooltip::Tooltip, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::keys::Layer;

use super::{
    LightingPage,
    controls::{ICON_SIZE, LABEL_TEXT, kebab},
};
use crate::dropdown::{Dropdown, Item};

const HEADER_TEXT: f32 = 16.0;
const HEADER_LINE: f32 = 22.4;
const PILL_HEIGHT: f32 = 38.0;
const PILL_LINE: f32 = 21.0;
const PILL_RADIUS: f32 = 6.0;
const MENU_WIDTH: f32 = 220.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Copy,
    Paste,
    UseMain,
}

impl LightingPage {
    pub(super) fn layers_column(&self, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        v_flex()
            .w_full()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(HEADER_TEXT))
                            .line_height(px(HEADER_LINE))
                            .text_color(theme.group_box_foreground)
                            .child("LAYERS"),
                    )
                    .child(
                        div()
                            .id("color-layers-help")
                            .text_color(theme.muted_foreground)
                            .child(Icon::new(Catalog::CircleQuestionMark).size(px(ICON_SIZE)))
                            .tooltip(|window, cx| {
                                Tooltip::new(
                                    "Functional layers are active while you hold the Fn key.",
                                )
                                .build(window, cx)
                            }),
                    ),
            )
            .child(
                v_flex().w_full().gap_2().children(
                    Layer::ALL
                        .into_iter()
                        .map(|layer| self.layer_pill(layer, cx)),
                ),
            )
    }

    fn layer_pill(&self, layer: Layer, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let own = self.map(layer).is_some();
        let inherits = !own && layer != Layer::Base;
        let selected = self.layer == layer;
        let hover = theme.secondary_hover;
        h_flex()
            .id(SharedString::from(format!(
                "color-layer-{}",
                layer.number()
            )))
            .w_full()
            .h(px(PILL_HEIGHT))
            .pl_3()
            .pr(px(1.0))
            .gap_1()
            .items_center()
            .justify_between()
            .rounded(px(PILL_RADIUS))
            .border_1()
            .border_color(if selected {
                theme.primary
            } else {
                theme.transparent
            })
            .bg(theme.secondary)
            .text_size(px(LABEL_TEXT))
            .line_height(px(PILL_LINE))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if inherits {
                theme.muted_foreground
            } else {
                theme.foreground
            })
            .cursor_pointer()
            .when(!selected, |this| {
                this.hover(move |style| style.border_color(hover))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.layer = layer;
                cx.notify();
            }))
            .child(div().min_w_0().truncate().child(layer_name(layer)))
            .child(self.layer_menu(layer, own, cx))
    }

    fn layer_menu(&self, layer: Layer, own: bool, cx: &Context<Self>) -> impl IntoElement {
        let page = cx.entity().downgrade();
        let mut actions = vec![Action::Copy, Action::Paste];
        if layer != Layer::Base {
            actions.push(Action::UseMain);
        }
        let items = actions.iter().map(|action| {
            let enabled = match action {
                Action::Copy => self.maps.is_some(),
                Action::Paste => self.copied.is_some(),
                Action::UseMain => own,
            };
            let item = Item::new(action.label())
                .disabled(!enabled)
                .danger(*action == Action::UseMain);
            if *action == Action::UseMain {
                item.divider_before()
            } else {
                item
            }
        });
        div().flex_none().child(
            Dropdown::new(SharedString::from(format!(
                "color-layer-menu-{}",
                layer.number()
            )))
            .trigger(kebab)
            .menu_width(px(MENU_WIDTH))
            .items(items.collect::<Vec<_>>())
            .on_select(move |index, window, cx| {
                let Some(action) = actions.get(index).copied() else {
                    return;
                };
                page.update(cx, |page, cx| match action {
                    Action::Copy => page.copy_layer(layer, cx),
                    Action::Paste => page.paste_layer(layer, window, cx),
                    Action::UseMain => page.clear_layer(layer, window, cx),
                })
                .ok();
            }),
        )
    }
}

impl Action {
    const fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Paste => "Paste and replace",
            Self::UseMain => "Use Main Layer colours",
        }
    }
}

const fn layer_name(layer: Layer) -> &'static str {
    match layer {
        Layer::Base => "Main Layer",
        Layer::One => "Fn Layer 1",
        Layer::Two => "Fn Layer 2",
        Layer::Three => "Fn Layer 3",
    }
}
