use super::pins::ALTERNATE_FUNCTION;
use crate::{
    gpio::{Level, Line},
    rcu::Hertz,
};

const CLOCKS: usize = 9;
const HALF_PERIOD_NANOSECONDS: u64 = 5_000;
const NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;

pub(super) trait Lines {
    fn drive_clock(&mut self, level: Level);
    fn drive_data(&mut self, level: Level);
    fn data(&self) -> Level;
    fn pause(&self);
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Pins {
    clock: Line,
    data: Line,
    half_period: u32,
}

impl Pins {
    pub(super) fn new(clock: Line, data: Line, core_clock: Hertz) -> Self {
        let cycles = HALF_PERIOD_NANOSECONDS * u64::from(core_clock.0) / NANOSECONDS_PER_SECOND;
        Self {
            clock,
            data,
            half_period: u32::try_from(cycles).unwrap_or(u32::MAX),
        }
    }

    pub(super) fn release(mut self) {
        self.clock.into_open_drain(Level::High);
        self.data.into_open_drain(Level::High);
        clear(&mut self);
        self.clock.into_alternate_open_drain(ALTERNATE_FUNCTION);
        self.data.into_alternate_open_drain(ALTERNATE_FUNCTION);
    }
}

impl Lines for Pins {
    fn drive_clock(&mut self, level: Level) {
        match level {
            Level::High => self.clock.set_high(),
            Level::Low => self.clock.set_low(),
        }
    }

    fn drive_data(&mut self, level: Level) {
        match level {
            Level::High => self.data.set_high(),
            Level::Low => self.data.set_low(),
        }
    }

    fn data(&self) -> Level {
        self.data.level()
    }

    fn pause(&self) {
        cortex_m::asm::delay(self.half_period);
    }
}

pub(super) fn clear(lines: &mut impl Lines) {
    if lines.data() == Level::High {
        return;
    }
    for _ in 0..CLOCKS {
        lines.drive_clock(Level::Low);
        lines.pause();
        lines.drive_clock(Level::High);
        lines.pause();
        if lines.data() == Level::High {
            break;
        }
    }
    lines.drive_clock(Level::Low);
    lines.pause();
    lines.drive_data(Level::Low);
    lines.pause();
    lines.drive_clock(Level::High);
    lines.pause();
    lines.drive_data(Level::High);
    lines.pause();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Condition {
        Start,
        Stop,
    }

    struct Bus {
        clock: Level,
        driven_data: Level,
        held_for_falling_clocks: usize,
        falling_clocks: usize,
        conditions: Vec<Condition>,
    }

    impl Bus {
        fn held_for(falling_clocks: usize) -> Self {
            Self {
                clock: Level::High,
                driven_data: Level::High,
                held_for_falling_clocks: falling_clocks,
                falling_clocks: 0,
                conditions: Vec::new(),
            }
        }

        fn target_holds_data(&self) -> bool {
            self.falling_clocks < self.held_for_falling_clocks
        }
    }

    impl Lines for Bus {
        fn drive_clock(&mut self, level: Level) {
            if self.clock == Level::High && level == Level::Low {
                self.falling_clocks += 1;
            }
            self.clock = level;
        }

        fn drive_data(&mut self, level: Level) {
            let before = self.data();
            self.driven_data = level;
            if self.clock == Level::High {
                match (before, self.data()) {
                    (Level::High, Level::Low) => self.conditions.push(Condition::Start),
                    (Level::Low, Level::High) => self.conditions.push(Condition::Stop),
                    _ => {}
                }
            }
        }

        fn data(&self) -> Level {
            if self.target_holds_data() {
                Level::Low
            } else {
                self.driven_data
            }
        }

        fn pause(&self) {}
    }

    #[test]
    fn a_free_data_line_is_left_alone() {
        let mut bus = Bus::held_for(0);
        clear(&mut bus);
        assert_eq!(bus.falling_clocks, 0);
        assert_eq!(bus.conditions, []);
    }

    #[test]
    fn a_target_holding_the_data_line_is_clocked_free_and_the_bus_stopped() {
        let mut bus = Bus::held_for(3);
        clear(&mut bus);
        assert!(!bus.target_holds_data());
        assert_eq!(bus.falling_clocks, 4);
        assert_eq!(bus.conditions, [Condition::Stop]);
        assert_eq!((bus.clock, bus.data()), (Level::High, Level::High));
    }

    #[test]
    fn a_target_that_keeps_holding_the_data_line_gets_nine_clocks() {
        let mut bus = Bus::held_for(usize::MAX);
        clear(&mut bus);
        assert_eq!(bus.falling_clocks, CLOCKS + 1);
        assert_eq!(bus.conditions, []);
        assert_eq!(bus.data(), Level::Low);
    }

    #[test]
    fn the_clock_half_period_follows_the_core_clock() {
        let pins = Pins::new(Line::new('B', 6), Line::new('B', 7), Hertz::from_mhz(200));
        assert_eq!(pins.half_period, 1000);
    }
}
