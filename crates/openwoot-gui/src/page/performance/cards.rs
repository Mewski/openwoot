use gpui_kit::{
    Div, IntoElement, ParentElement as _, Styled as _, component::h_flex, div,
    prelude::FluentBuilder as _, px,
};

const MIN_HEIGHT: f32 = 360.0;
const DISABLED_OPACITY: f32 = 0.64;
const ART_SLOT: f32 = 104.0;

pub(crate) fn tuning_card(card: Div, enabled: bool) -> Div {
    card.min_h(px(MIN_HEIGHT))
        .when(!enabled, |this| this.opacity(DISABLED_OPACITY))
}

pub(crate) fn card_row() -> Div {
    h_flex().w_full().items_stretch().gap_4()
}

pub(crate) fn art_slot(art: impl IntoElement) -> Div {
    div()
        .flex_none()
        .size(px(ART_SLOT))
        .flex()
        .items_center()
        .justify_center()
        .child(art)
}
