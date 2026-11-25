use gpui_kit::{Hsla, rgb};

use crate::theme;

const SHORTEST: u32 = 0x004b_44be;
const FADE: f32 = 1.6;

pub(super) fn of(fraction: f32) -> Hsla {
    let strength = (1.0 - fraction.clamp(0.0, 1.0)).powf(FADE);
    theme::keycaps()
        .top
        .blend(Hsla::from(rgb(SHORTEST)).opacity(strength))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shorter_distances_are_brighter() {
        let shortest = of(0.0);
        let middle = of(0.4);
        let longest = of(1.0);
        assert!(shortest.l > middle.l);
        assert!(middle.l > longest.l);
        assert_eq!(longest, theme::keycaps().top);
        assert_eq!(of(-1.0), shortest);
    }
}
