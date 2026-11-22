use super::calibration::VALUE_NONE;

const PRESS_PERCENT: u32 = 30;
const BOUNCE_PERCENT: u32 = 10;
const REST_TIMEOUT_US: u16 = 3500;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReboundState {
    #[default]
    Idle,
    Pressed,
    Releasing,
    Released,
    Rebound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rebound {
    previous_reading: u16,
    rest_timer_running: bool,
    rest_timer_start_us: u16,
    state: ReboundState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trend {
    Stable,
    Rising,
    Falling,
}

impl Default for Rebound {
    fn default() -> Self {
        Self::new()
    }
}

impl Rebound {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            previous_reading: VALUE_NONE,
            rest_timer_running: false,
            rest_timer_start_us: 0,
            state: ReboundState::Idle,
        }
    }

    #[must_use]
    pub const fn state(&self) -> ReboundState {
        self.state
    }

    pub fn detect(
        &mut self,
        rest_level: u16,
        bottom_level: u16,
        reading: u16,
        now_us: u16,
    ) -> bool {
        let rest = u32::from(rest_level);
        let bottom = u32::from(bottom_level);
        let reading_wide = u32::from(reading);
        let clamped = rest.max(reading_wide);
        let depth = if clamped > bottom {
            bottom.wrapping_sub(rest)
        } else {
            clamped - rest
        };
        let range = bottom.wrapping_sub(rest);
        let travel_percent = (100_u32.wrapping_mul(depth))
            .checked_div(range)
            .map_or(0, |percent| percent & 0xFFFF);
        let previous = u32::from(self.previous_reading);
        let trend = if previous.wrapping_sub(reading_wide).wrapping_add(1) <= 2 {
            Trend::Stable
        } else if previous < reading_wide {
            Trend::Rising
        } else {
            Trend::Falling
        };
        self.previous_reading = reading;

        match self.state {
            ReboundState::Idle => {
                if travel_percent > PRESS_PERCENT {
                    self.state = ReboundState::Pressed;
                }
                false
            }
            ReboundState::Pressed => {
                if trend == Trend::Falling {
                    self.state = ReboundState::Releasing;
                }
                false
            }
            ReboundState::Releasing => {
                if travel_percent == 0 {
                    self.state = ReboundState::Released;
                } else if trend == Trend::Rising {
                    self.state = ReboundState::Idle;
                }
                false
            }
            ReboundState::Released => {
                if trend == Trend::Rising {
                    self.rest_timer_running = false;
                    self.state = ReboundState::Rebound;
                    return true;
                }
                if rest < reading_wide {
                    self.rest_timer_running = false;
                } else if !self.rest_timer_running {
                    self.rest_timer_running = true;
                    self.rest_timer_start_us = now_us;
                } else if self.rest_timed_out(now_us) {
                    self.rest_timer_running = false;
                    self.state = ReboundState::Idle;
                }
                false
            }
            ReboundState::Rebound => {
                if travel_percent > BOUNCE_PERCENT {
                    self.state = ReboundState::Idle;
                    self.rest_timer_running = false;
                    return false;
                }
                if rest < reading_wide {
                    self.rest_timer_running = false;
                    return true;
                }
                if !self.rest_timer_running {
                    self.rest_timer_running = true;
                    self.rest_timer_start_us = now_us;
                    return true;
                }
                if self.rest_timed_out(now_us) {
                    self.rest_timer_running = false;
                    self.state = ReboundState::Idle;
                    return false;
                }
                true
            }
        }
    }

    const fn rest_timed_out(self, now_us: u16) -> bool {
        now_us.wrapping_sub(self.rest_timer_start_us) > REST_TIMEOUT_US - 1
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    const REST: u16 = 105;
    const BOTTOM: u16 = 1105;

    fn feed(filter: &mut Rebound, readings: &[(u16, u16)]) -> Vec<bool> {
        readings
            .iter()
            .map(|&(reading, now_us)| filter.detect(REST, BOTTOM, reading, now_us))
            .collect()
    }

    #[test]
    fn a_bounce_after_a_full_release_is_suppressed() {
        let mut filter = Rebound::new();
        let reported = feed(
            &mut filter,
            &[(800, 0), (700, 100), (REST, 200), (150, 300), (160, 400)],
        );
        assert_eq!(reported, [false, false, false, true, true]);
        assert_eq!(filter.state(), ReboundState::Rebound);
    }

    #[test]
    fn a_real_press_after_a_bounce_ends_the_suppression() {
        let mut filter = Rebound::new();
        feed(
            &mut filter,
            &[(800, 0), (700, 100), (REST, 200), (150, 300)],
        );
        assert!(!filter.detect(REST, BOTTOM, 300, 400));
        assert_eq!(filter.state(), ReboundState::Idle);
    }

    #[test]
    fn the_filter_disarms_after_three_and_a_half_milliseconds_at_rest() {
        let mut filter = Rebound::new();
        feed(
            &mut filter,
            &[(800, 0), (700, 100), (REST, 200), (REST, 300)],
        );
        assert!(!filter.detect(REST, BOTTOM, REST, 3799));
        assert_eq!(filter.state(), ReboundState::Released);
        assert!(!filter.detect(REST, BOTTOM, REST, 3800));
        assert_eq!(filter.state(), ReboundState::Idle);
    }

    #[test]
    fn pressing_again_before_rest_rearms_the_filter() {
        let mut filter = Rebound::new();
        feed(&mut filter, &[(800, 0), (700, 100)]);
        assert!(!filter.detect(REST, BOTTOM, 750, 200));
        assert_eq!(filter.state(), ReboundState::Idle);
    }

    #[test]
    fn a_zero_range_reads_as_no_travel() {
        let mut filter = Rebound::new();
        assert!(!filter.detect(REST, REST, 900, 0));
        assert_eq!(filter.state(), ReboundState::Idle);
    }
}
