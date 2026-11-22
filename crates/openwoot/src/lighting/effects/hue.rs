use openwoot_protocol::lighting::{HueRange, Rgb};

use crate::lighting::color::rgb;

const SECTOR_WIDTH: u8 = 43;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Hues {
    pub(super) first: u8,
    pub(super) last: u8,
}

impl Hues {
    pub(super) const fn of(range: HueRange) -> Self {
        let (first, last) = match range {
            HueRange::FullCircle => (0, 255),
            HueRange::From110To184 => (110, 184),
            HueRange::From2To39 => (2, 39),
            HueRange::From64To114 => (64, 114),
            HueRange::From214To254 => (214, 254),
            HueRange::From113To204 => (113, 204),
        };
        Self { first, last }
    }
}

pub(super) const fn hue_to_rgb(hue: u8) -> Rgb {
    let remainder = (hue % SECTOR_WIDTH) as i32 * 6;
    let falling = ((255 * (255 - ((255 * remainder) >> 8))) >> 8).to_le_bytes()[0];
    let rising = ((255 * (255 - ((255 * (255 - remainder)) >> 8))) >> 8).to_le_bytes()[0];
    match hue / SECTOR_WIDTH {
        0 => rgb(255, rising, 0),
        1 => rgb(falling, 255, 0),
        2 => rgb(0, 255, rising),
        3 => rgb(0, falling, 255),
        4 => rgb(rising, 0, 255),
        _ => rgb(255, 0, falling),
    }
}

pub(super) const fn phase_step(period: u8, elapsed_ms: u32) -> u16 {
    let per_millisecond = match 0x1_0000_u32.checked_div(250 * period as u32) {
        Some(step) => step,
        None => 0,
    };
    let [low, high, _, _] = per_millisecond.wrapping_mul(elapsed_ms).to_le_bytes();
    u16::from_le_bytes([low, high])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_six_sectors_run_through_the_colour_circle() {
        assert_eq!(hue_to_rgb(0), rgb(255, 0, 0));
        assert_eq!(hue_to_rgb(42), rgb(255, 252, 0));
        assert_eq!(hue_to_rgb(43), rgb(254, 255, 0));
        assert_eq!(hue_to_rgb(86), rgb(0, 255, 0));
        assert_eq!(hue_to_rgb(129), rgb(0, 254, 255));
        assert_eq!(hue_to_rgb(172), rgb(0, 0, 255));
        assert_eq!(hue_to_rgb(215), rgb(255, 0, 254));
        assert_eq!(hue_to_rgb(255), rgb(255, 0, 15));
    }

    #[test]
    fn a_period_of_zero_freezes_the_phase() {
        assert_eq!(phase_step(0, 16), 0);
        assert_eq!(phase_step(4, 16), 16 * 65);
        assert_eq!(phase_step(1, 1000), 0xFF70);
    }
}
