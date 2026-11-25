#![allow(
    clippy::inline_always,
    reason = "the level writes run inside the scan interrupt, inlined to keep it short"
)]

use super::{
    Level, Speed,
    registers::{
        self, ALTERNATE_OPEN_DRAIN, INPUT_ANALOG, OUTPUT_OPEN_DRAIN, OUTPUT_PUSH_PULL, PINS,
        enable_clock, is_port, level_of, port_index, set_mode, set_reset,
    },
};
use crate::register::{read_bits, write_bits};

const BACKUP_DOMAIN_PORT: u8 = port_index('C');
const BACKUP_DOMAIN_FIRST_PIN: u8 = 13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    port: u8,
    pin: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Levels {
    port: u8,
    set_reset: u32,
}

impl Line {
    #[must_use]
    pub const fn new(port: char, pin: u8) -> Self {
        assert!(Self::exists(port, pin), "a pin of ports A to G");
        Self::from_index(port_index(port), pin)
    }

    #[must_use]
    pub const fn exists(port: char, pin: u8) -> bool {
        is_port(port) && pin < PINS
    }

    pub(crate) const fn from_index(port: u8, pin: u8) -> Self {
        Self { port, pin }
    }

    pub(crate) fn into_output(self, level: Level) {
        self.into_driven(OUTPUT_PUSH_PULL, level);
    }

    pub(crate) fn into_open_drain(self, level: Level) {
        self.into_driven(OUTPUT_OPEN_DRAIN, level);
    }

    pub(crate) fn into_alternate_open_drain(self) {
        enable_clock(self.port);
        self.configure(ALTERNATE_OPEN_DRAIN | self.output_speed().mode());
    }

    pub(crate) fn into_analog(self) {
        enable_clock(self.port);
        self.configure(INPUT_ANALOG);
    }

    const fn output_speed(self) -> Speed {
        if self.port == BACKUP_DOMAIN_PORT && self.pin >= BACKUP_DOMAIN_FIRST_PIN {
            Speed::Low
        } else {
            Speed::High
        }
    }

    #[inline(always)]
    pub(crate) fn set_high(self) {
        write_bits(registers::port(self.port).bop(), self.mask());
    }

    #[inline(always)]
    pub(crate) fn set_low(self) {
        write_bits(registers::port(self.port).bc(), self.mask());
    }

    #[inline(always)]
    const fn mask(self) -> u32 {
        1 << self.pin
    }

    pub(crate) fn level(self) -> Level {
        level_of(read_bits(registers::port(self.port).istat()), self.pin)
    }

    fn into_driven(self, control: u32, level: Level) {
        enable_clock(self.port);
        match level {
            Level::High => self.set_high(),
            Level::Low => self.set_low(),
        }
        self.configure(control | self.output_speed().mode());
    }

    fn configure(self, nibble: u32) {
        critical_section::with(|_| set_mode(registers::port(self.port), self.pin, nibble));
    }
}

impl Levels {
    pub(crate) fn new(port: u8, pins: impl IntoIterator<Item = (u8, Level)>) -> Self {
        let set_reset = pins
            .into_iter()
            .fold(0, |bits, (pin, level)| bits | set_reset(pin, level));
        Self { port, set_reset }
    }

    #[inline(always)]
    pub(crate) fn apply(self) {
        write_bits(registers::port(self.port).bop(), self.set_reset);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_count_ports_from_a() {
        assert_eq!(Line::new('C', 5), Line::from_index(2, 5));
    }

    #[test]
    fn only_pins_of_ports_a_to_g_exist() {
        assert!(Line::exists('A', 0) && Line::exists('G', 15));
        assert!(!Line::exists('H', 0) && !Line::exists('B', 16) && !Line::exists('\u{87}', 3));
    }

    #[test]
    fn the_pins_behind_the_backup_power_switch_are_driven_at_2_megahertz() {
        for pin in [13, 14, 15] {
            assert_eq!(Line::new('C', pin).output_speed(), Speed::Low);
        }
        assert_eq!(Line::new('C', 12).output_speed(), Speed::High);
        assert_eq!(Line::new('B', 13).output_speed(), Speed::High);
        assert_eq!(OUTPUT_PUSH_PULL | Speed::Low.mode(), 0x2);
    }

    #[test]
    fn levels_set_the_high_pins_and_reset_the_low_ones() {
        let levels = Levels::new(
            0,
            [
                (0, Level::Low),
                (1, Level::High),
                (10, Level::High),
                (11, Level::Low),
            ],
        );
        assert_eq!(levels.set_reset, 0x0402 | 0x0801 << 16);
    }
}
