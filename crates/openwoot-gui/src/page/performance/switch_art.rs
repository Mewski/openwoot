use gpui_kit::{
    App, Div, Hsla, ParentElement as _, Styled as _, component::ActiveTheme as _, div, px,
};

const WIDTH: f32 = 96.0;
const HEIGHT: f32 = 112.0;
const STEM_WIDTH: f32 = 22.0;
const STEM_HEIGHT: f32 = 40.0;
const STEM_TOP: f32 = 4.0;
const STEM_TRAVEL: f32 = 24.0;
const HOUSING_TOP: f32 = 30.0;
const HOUSING_WIDTH: f32 = 76.0;
const HOUSING_HEIGHT: f32 = 34.0;
const BASE_WIDTH: f32 = 64.0;
const BASE_HEIGHT: f32 = 22.0;
const PIN_WIDTH: f32 = 4.0;
const PIN_HEIGHT: f32 = 16.0;
const PIN_INSET: f32 = 10.0;
const SPRING_COILS: u8 = 5;
const SPRING_WIDTH: f32 = 30.0;

pub(crate) fn switch(depth: f32, stem: Hsla, cx: &App) -> Div {
    let housing = cx.theme().muted_foreground;
    let depth = depth.clamp(0.0, 1.0);
    let base_left = (WIDTH - BASE_WIDTH) / 2.0;
    let base_top = HOUSING_TOP + HOUSING_HEIGHT;
    let stem_top = STEM_TOP + depth * STEM_TRAVEL;
    let spring_top = stem_top + STEM_HEIGHT - 8.0;
    let spring_bottom = base_top + BASE_HEIGHT - 6.0;
    let coil_gap = (spring_bottom - spring_top) / f32::from(SPRING_COILS);
    div()
        .relative()
        .flex_none()
        .w(px(WIDTH))
        .h(px(HEIGHT))
        .child(
            div()
                .absolute()
                .left(px((WIDTH - STEM_WIDTH) / 2.0))
                .top(px(stem_top))
                .w(px(STEM_WIDTH))
                .h(px(STEM_HEIGHT))
                .rounded_t(px(4.0))
                .bg(stem),
        )
        .children((0..SPRING_COILS).map(|coil| {
            div()
                .absolute()
                .left(px((WIDTH - SPRING_WIDTH) / 2.0))
                .top(px(spring_top + coil_gap * f32::from(coil)))
                .w(px(SPRING_WIDTH))
                .h(px(2.0))
                .rounded_full()
                .bg(housing.opacity(0.7))
        }))
        .child(
            div()
                .absolute()
                .left(px((WIDTH - HOUSING_WIDTH) / 2.0))
                .top(px(HOUSING_TOP))
                .w(px(HOUSING_WIDTH))
                .h(px(HOUSING_HEIGHT))
                .rounded(px(6.0))
                .bg(housing.opacity(0.35))
                .border_1()
                .border_color(housing.opacity(0.6)),
        )
        .child(
            div()
                .absolute()
                .left(px(base_left))
                .top(px(base_top))
                .w(px(BASE_WIDTH))
                .h(px(BASE_HEIGHT))
                .rounded_b(px(6.0))
                .bg(housing.opacity(0.55)),
        )
        .children(
            [
                base_left + PIN_INSET,
                base_left + BASE_WIDTH - PIN_INSET - PIN_WIDTH,
            ]
            .map(|left| {
                div()
                    .absolute()
                    .left(px(left))
                    .top(px(base_top + BASE_HEIGHT))
                    .w(px(PIN_WIDTH))
                    .h(px(PIN_HEIGHT))
                    .rounded_b(px(2.0))
                    .bg(housing)
            }),
        )
}
