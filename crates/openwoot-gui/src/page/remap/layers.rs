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

const PILL_HEIGHT: f32 = 38.0;
const PILL_GAP: f32 = 10.0;
const PILL_RADIUS: f32 = 6.0;
const LABEL_GAP: f32 = 3.0;
const ADD_GAP: f32 = 15.0;
const HELP_ICON: f32 = 20.0;
const MENU_ICON: f32 = 20.0;
const LIMIT_OPACITY: f32 = 0.5;
const LAYERS_HELP: &str = "Hold a Fn key to use its layer.";
const LIMIT_REACHED: &str = "Maximum layer limit reached.";

pub(crate) const fn layer_label(layer: Layer) -> &'static str {
    match layer {
        Layer::Base => "Main Layer",
        Layer::One => "Fn Layer 1",
        Layer::Two => "Fn Layer 2",
        Layer::Three => "Fn Layer 3",
    }
}

pub(crate) fn column(
    pills: impl IntoIterator<Item = AnyElement>,
    footer: Option<AnyElement>,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    v_flex()
        .w_full()
        .gap(px(PILL_GAP))
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .pb(px(LABEL_GAP))
                .child(
                    div()
                        .text_base()
                        .text_color(theme.group_box_foreground)
                        .child("LAYERS"),
                )
                .child(
                    div()
                        .id("layers-help")
                        .text_color(theme.muted_foreground)
                        .child(Icon::new(Catalog::CircleQuestionMark).size(px(HELP_ICON)))
                        .tooltip(|window, cx| Tooltip::new(LAYERS_HELP).build(window, cx)),
                ),
        )
        .children(pills)
        .children(footer.map(|footer| div().pt(px(ADD_GAP)).child(footer)))
}

pub(crate) fn pill(
    id: impl Into<SharedString>,
    layer: Layer,
    selected: bool,
    trailing: Option<AnyElement>,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.secondary;
    let padded = trailing.is_none();
    h_flex()
        .id(id.into())
        .w_full()
        .h(px(PILL_HEIGHT))
        .pl_3()
        .map(|this| {
            if padded {
                this.pr_3()
            } else {
                this.pr(px(1.0))
            }
        })
        .gap_1()
        .items_center()
        .justify_between()
        .rounded(px(PILL_RADIUS))
        .border_1()
        .border_color(if selected {
            theme.primary
        } else {
            theme.accent
        })
        .bg(if selected {
            theme.secondary
        } else {
            theme.accent
        })
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
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
        .rounded(px(PILL_RADIUS))
        .bg(theme.secondary)
        .text_sm()
        .font_weight(FontWeight::SEMIBOLD)
        .map(|this| {
            if limit_reached {
                this.opacity(LIMIT_OPACITY)
                    .tooltip(|window, cx| Tooltip::new(LIMIT_REACHED).build(window, cx))
            } else {
                this.cursor_pointer().hover(move |style| style.bg(hover))
            }
        })
        .child("Add Layer")
        .child(Icon::new(Catalog::Plus).size(px(MENU_ICON)))
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
