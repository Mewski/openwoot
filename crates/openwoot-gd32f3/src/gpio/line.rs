#![allow(
    clippy::inline_always,
    reason = "the level writes run in the scan interrupt, inlined as the C HAL's always_inline \
              helpers"
)]

use super::{
    pin::{Level, Speed},
    registers::{self, INPUT_ANALOG, OUTPUT_PUSH_PULL, enable_clock, set_mode, set_reset},
};
use crate::register::write_bits;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    port: u8,
    pin: u8,
}

impl Line {
    #[must_use]
    pub const fn new(port: char, pin: u8) -> Self {
        assert!(
            port >= 'A' && port <= 'G' && pin < 16,
            "a pin of ports A to G"
        );
        Self {
            port: registers::port_index(port),
            pin,
        }
    }

    pub(crate) const fn from_index(port: u8, pin: u8) -> Self {
        Self { port, pin }
    }

    pub(crate) fn into_output(self, level: Level) {
        self.configure(OUTPUT_PUSH_PULL | Speed::High.mode());
        match level {
            Level::High => self.set_high(),
            Level::Low => self.set_low(),
        }
    }

    pub(crate) fn into_analog(self) {
        self.configure(INPUT_ANALOG);
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

    fn configure(self, nibble: u32) {
        enable_clock(self.port);
        critical_section::with(|_| set_mode(registers::port(self.port), self.pin, nibble));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Levels {
    port: u8,
    set_reset: u32,
}

impl Levels {
    pub(crate) fn new(port: u8, pins: impl IntoIterator<Item = (u8, Level)>) -> Self {
        Self {
            port,
            set_reset: pins
                .into_iter()
                .fold(0, |bits, (pin, level)| bits | set_reset(pin, level)),
        }
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
    fn lines_name_their_port_from_a() {
        assert_eq!(Line::new('C', 5), Line::from_index(2, 5));
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
