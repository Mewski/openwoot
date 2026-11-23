use core::num::NonZeroU32;

use openwoot_protocol::lighting::{HueRange, Rgb};

use crate::lighting::{color::rgb, truncate::low_16};

const SECTOR_WIDTH: u8 = 43;
const CYCLE_MS_PER_PERIOD: u32 = 250;
const TURN_UNITS: u32 = 0x1_0000;
const LONGEST_CYCLE_MS: u32 = CYCLE_MS_PER_PERIOD * u8::MAX as u32;
const _: () = assert!((LONGEST_CYCLE_MS - 1).checked_mul(TURN_UNITS + 1).is_some());

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Phase {
    turn: u16,
    carried_remainder: u16,
}

impl Phase {
    pub(super) const fn new() -> Self {
        Self {
            turn: 0,
            carried_remainder: 0,
        }
    }

    pub(super) const fn turn(self) -> u16 {
        self.turn
    }

    pub(super) fn advance(&mut self, period: u8, elapsed_ms: u32, forward: bool) {
        let Some(cycle_ms) = cycle_ms_of(period) else {
            return;
        };
        let elapsed_within_cycle_ms = elapsed_ms % cycle_ms;
        let carried = u32::from(self.carried_remainder).min(cycle_ms.get() - 1);
        let scaled_elapsed = elapsed_within_cycle_ms * TURN_UNITS + carried;
        let step = low_16(scaled_elapsed / cycle_ms);
        self.carried_remainder = low_16(scaled_elapsed % cycle_ms);
        self.turn = if forward {
            self.turn.wrapping_add(step)
        } else {
            self.turn.wrapping_sub(step)
        };
    }
}

fn cycle_ms_of(period: u8) -> Option<NonZeroU32> {
    NonZeroU32::new(CYCLE_MS_PER_PERIOD * u32::from(period))
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

    fn advanced(period: u8, forward: bool, steps: impl IntoIterator<Item = u32>) -> u16 {
        let mut phase = Phase::new();
        for elapsed_ms in steps {
            phase.advance(period, elapsed_ms, forward);
        }
        phase.turn()
    }

    fn exact_turn_rounded_down(period: u8, elapsed_ms: u64) -> u16 {
        let units = elapsed_ms * 0x1_0000 / (250 * u64::from(period));
        u16::try_from(units % 0x1_0000).unwrap()
    }

    #[test]
    fn a_period_of_zero_freezes_the_phase() {
        assert_eq!(advanced(0, true, [16, 1000, u32::MAX]), 0);
    }

    #[test]
    fn every_period_runs_at_its_own_speed_without_drift() {
        for period in 1..=u8::MAX {
            let cycle_ms = 250 * u32::from(period);
            assert_eq!(advanced(period, true, [cycle_ms]), 0, "period {period}");
            assert_eq!(
                advanced(period, true, [cycle_ms, cycle_ms / 4]),
                exact_turn_rounded_down(period, u64::from(cycle_ms / 4)),
                "period {period}"
            );
            for step_ms in [1, 4, 16, 33] {
                let steps = 100_000 / step_ms;
                assert_eq!(
                    advanced(period, true, (0..steps).map(|_| step_ms)),
                    exact_turn_rounded_down(period, u64::from(steps * step_ms)),
                    "period {period}, steps of {step_ms} ms"
                );
            }
        }
    }

    #[test]
    fn long_periods_differ_in_speed() {
        for period in 132..u8::MAX {
            assert!(
                advanced(period, true, [10_000]) > advanced(period + 1, true, [10_000]),
                "period {period}"
            );
        }
        assert_eq!(
            advanced(255, true, [10_000]),
            exact_turn_rounded_down(255, 10_000)
        );
    }

    #[test]
    fn a_backward_phase_mirrors_the_forward_one() {
        for period in [1, 4, 30, 65, 200] {
            let forward = advanced(period, true, [16; 500]);
            let backward = advanced(period, false, [16; 500]);
            assert_eq!(backward, forward.wrapping_neg(), "period {period}");
        }
    }
}
