use gpui_kit::{
    Context, Div, IntoElement, ParentElement as _, SharedString, StatefulInteractiveElement as _,
    Styled as _,
    component::{
        IconName, Sizable as _,
        button::{Button, ButtonVariants as _},
        menu::{DropdownMenu as _, PopupMenuItem},
        v_flex,
    },
};
use openwoot_protocol::keys::Layer;

use super::{LightingPage, pill};
use crate::panel;

impl LightingPage {
    pub(super) fn layers_column(&self, cx: &Context<Self>) -> Div {
        v_flex()
            .w_full()
            .gap_2()
            .child(panel::label("Layers", cx))
            .children(
                Layer::ALL
                    .into_iter()
                    .map(|layer| self.layer_pill(layer, cx)),
            )
    }

    fn layer_pill(&self, layer: Layer, cx: &Context<Self>) -> impl IntoElement {
        let own = self.map(layer).is_some();
        let inherited = !own && layer != Layer::Base;
        pill(
            format!("color-layer-{}", layer.number()),
            layer_name(layer),
            None,
            self.layer == layer,
            inherited,
            cx,
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.layer = layer;
            cx.notify();
        }))
        .child(Self::layer_menu(layer, own, cx))
    }

    fn layer_menu(layer: Layer, own: bool, cx: &Context<Self>) -> impl IntoElement + use<> {
        let page = cx.entity().downgrade();
        Button::new(SharedString::from(format!(
            "color-layer-menu-{}",
            layer.number()
        )))
        .icon(IconName::EllipsisVertical)
        .ghost()
        .xsmall()
        .dropdown_menu(move |menu, _, _| {
            let page = page.clone();
            menu.item(
                PopupMenuItem::new(clear_label(layer))
                    .disabled(!own)
                    .on_click(move |_, window, cx| {
                        page.update(cx, |page, cx| page.clear_layer(layer, window, cx))
                            .ok();
                    }),
            )
        })
    }
}

const fn layer_name(layer: Layer) -> &'static str {
    match layer {
        Layer::Base => "Main layer",
        Layer::One => "Fn layer 1",
        Layer::Two => "Fn layer 2",
        Layer::Three => "Fn layer 3",
    }
}

const fn clear_label(layer: Layer) -> &'static str {
    match layer {
        Layer::Base => "Turn off key colours",
        Layer::One | Layer::Two | Layer::Three => "Use main layer colours",
    }
}
