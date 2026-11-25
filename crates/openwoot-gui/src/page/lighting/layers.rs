use gpui_kit::{
    Context, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, StatefulInteractiveElement as _, Styled as _,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, Sizable as _, h_flex, tooltip::Tooltip, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::keys::Layer;

use super::{LightingPage, controls::kebab};
use crate::{
    dropdown::{Dropdown, Item},
    panel,
};

const PILL_HEIGHT: f32 = 40.0;
const MENU_WIDTH: f32 = 220.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Copy,
    Paste,
    UseMain,
}

impl LightingPage {
    pub(super) fn layers_column(&self, cx: &Context<Self>) -> Div {
        v_flex()
            .w_full()
            .gap_2()
            .child(
                h_flex()
                    .gap_1p5()
                    .items_center()
                    .pb_1()
                    .child(panel::label("Layers", cx))
                    .child(
                        div()
                            .id("color-layers-help")
                            .text_color(cx.theme().muted_foreground)
                            .child(Icon::new(Catalog::CircleQuestionMark).xsmall())
                            .tooltip(|window, cx| {
                                Tooltip::new(
                                    "An Fn layer shows its colours while it is held; without its own colours it shows the main layer's",
                                )
                                .build(window, cx)
                            }),
                    ),
            )
            .children(
                Layer::ALL
                    .into_iter()
                    .map(|layer| self.layer_pill(layer, cx)),
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
            .pr_1()
            .gap_1()
            .items_center()
            .justify_between()
            .rounded(theme.radius)
            .border_1()
            .border_color(if selected {
                theme.primary
            } else {
                theme.group_box
            })
            .bg(theme.group_box)
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
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
            Item::new(action.label())
                .disabled(!enabled)
                .danger(*action == Action::UseMain)
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
            Self::Copy => "Copy colours",
            Self::Paste => "Paste and replace",
            Self::UseMain => "Use main layer colours",
        }
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
