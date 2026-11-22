use crate::lighting::truncate::low_8;

pub(super) fn pick_press_color(last_color: &mut u8, now_ms: u32) -> u8 {
    let tick = low_8(now_ms & 7);
    let color = if tick + 1 > 6 {
        if *last_color == 7 { 1 } else { 7 }
    } else if *last_color == tick + 1 {
        if *last_color == 7 { 1 } else { tick + 2 }
    } else {
        tick + 1
    };
    *last_color = color;
    color & 0x07
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_colours_never_repeat_the_previous_one() {
        let mut last = 0;
        assert_eq!(pick_press_color(&mut last, 0), 1);
        assert_eq!(pick_press_color(&mut last, 8), 2);
        assert_eq!(pick_press_color(&mut last, 6), 7);
        assert_eq!(pick_press_color(&mut last, 7), 1);
        assert_eq!(pick_press_color(&mut last, 0), 2);
        assert_eq!(pick_press_color(&mut last, 5), 6);
        assert_eq!(pick_press_color(&mut last, 5), 7);
        for now in 0..64 {
            let previous = last;
            let color = pick_press_color(&mut last, now);
            assert!((1..=7).contains(&color));
            assert_ne!(color, previous);
        }
    }
}
