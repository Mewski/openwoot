use gpui_kit::{Div, Hsla, ParentElement as _, Pixels, Styled as _, div, hsla, px};

const FIRST_HUE: f32 = 0.47;
const HUE_STEP: f32 = 0.618_034;
const STEM_SATURATION: f32 = 0.5;
const STEM_LIGHTNESS: f32 = 0.56;

pub(super) fn stem_color(model: u8) -> Hsla {
    hsla(
        (FIRST_HUE + f32::from(model) * HUE_STEP).fract(),
        STEM_SATURATION,
        STEM_LIGHTNESS,
        1.0,
    )
}

pub(super) fn switch(size: Pixels, stem: Hsla, housing: Hsla) -> Div {
    let length = f32::from(size);
    let at = |share: f32| px(length * share);
    let part = |left: f32, top: f32, width: f32, height: f32| {
        div()
            .absolute()
            .left(at(left))
            .top(at(top))
            .w(at(width))
            .h(at(height))
    };
    div()
        .relative()
        .flex_none()
        .size(size)
        .child(
            part(0.07, 0.07, 0.86, 0.86)
                .rounded(at(0.16))
                .bg(housing)
                .border_1()
                .border_color(housing.opacity(0.8)),
        )
        .child(
            part(0.2, 0.2, 0.6, 0.6)
                .rounded(at(0.1))
                .bg(stem.opacity(0.35)),
        )
        .child(
            part(0.4, 0.1, 0.2, 0.06)
                .rounded(at(0.03))
                .bg(housing.opacity(0.9)),
        )
        .child(part(0.31, 0.45, 0.38, 0.1).rounded(at(0.02)).bg(stem))
        .child(part(0.45, 0.31, 0.1, 0.38).rounded(at(0.02)).bg(stem))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbouring_models_get_distinct_stems() {
        let colors: Vec<Hsla> = (0..8).map(stem_color).collect();
        for (index, color) in colors.iter().enumerate() {
            for other in colors.iter().skip(index + 1) {
                assert!((color.h - other.h).abs() > 0.05);
            }
        }
    }
}
