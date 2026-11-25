use gpui_kit::{
    AnyElement, App, Div, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled as _,
    assets::IconName as Catalog,
    component::{ActiveTheme as _, Icon, Sizable as _, h_flex, tooltip::Tooltip, v_flex},
    div,
    prelude::FluentBuilder as _,
    px,
};
use openwoot_protocol::keys::Layer;

use crate::panel;

const PILL_HEIGHT: f32 = 38.0;
const LAYERS_HELP: &str = "Fn layers hold extra keys and functions; hold a Fn key to use them";

pub(crate) const fn layer_label(layer: Layer) -> &'static str {
    match layer {
        Layer::Base => "Main layer",
        Layer::One => "Fn layer 1",
        Layer::Two => "Fn layer 2",
        Layer::Three => "Fn layer 3",
    }
}

pub(crate) fn column(
    pills: impl IntoIterator<Item = AnyElement>,
    footer: Option<AnyElement>,
    cx: &App,
) -> Div {
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
                        .id("layers-help")
                        .text_color(cx.theme().muted_foreground)
                        .child(Icon::new(Catalog::CircleQuestionMark).xsmall())
                        .tooltip(|window, cx| Tooltip::new(LAYERS_HELP).build(window, cx)),
                ),
        )
        .children(pills)
        .children(footer.map(|footer| div().pt_2().child(footer)))
}

pub(crate) fn pill(
    id: impl Into<SharedString>,
    layer: Layer,
    selected: bool,
    trailing: Option<AnyElement>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.secondary_hover;
    let padded = trailing.is_none();
    h_flex()
        .id(id.into())
        .w_full()
        .h(px(PILL_HEIGHT))
        .pl_3()
        .map(|this| if padded { this.pr_3() } else { this.pr_1() })
        .gap_2()
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
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .child(layer_label(layer))
        .children(trailing)
}

pub(crate) fn add_button(
    id: impl Into<SharedString>,
    limit_reached: bool,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.secondary_hover;
    h_flex()
        .id(id.into())
        .w_full()
        .h(px(PILL_HEIGHT))
        .px_3()
        .items_center()
        .justify_between()
        .rounded(theme.radius)
        .bg(theme.secondary)
        .text_sm()
        .font_weight(FontWeight::MEDIUM)
        .map(|this| {
            if limit_reached {
                this.opacity(0.5).tooltip(|window, cx| {
                    Tooltip::new("Maximum layer limit reached").build(window, cx)
                })
            } else {
                this.cursor_pointer().hover(move |style| style.bg(hover))
            }
        })
        .child("Add layer")
        .child(Icon::new(Catalog::Plus).small())
}

pub(crate) fn warning(id: impl Into<SharedString>, text: &'static str, cx: &App) -> AnyElement {
    div()
        .id(id.into())
        .flex_none()
        .text_color(cx.theme().warning)
        .child(Icon::new(Catalog::TriangleAlert).xsmall())
        .tooltip(move |window, cx| Tooltip::new(text).build(window, cx))
        .into_any_element()
}
