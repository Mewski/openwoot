#![allow(
    clippy::inline_always,
    reason = "the level writes run inside the scan interrupt as the C HAL's always_inline helpers"
)]

use super::{
    Level, OutputType, Pull, Speed,
    registers::{
        self, MODE_ANALOG, MODE_OUTPUT, enable_clock, modify_output_type, modify_two_bit_field,
        port_index, set_reset,
    },
};
use crate::register::write_bits;

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
        assert!(
            port >= 'A' && port <= 'I' && pin < 16,
            "a pin of ports A to I"
        );
        Self::from_index(port_index(port), pin)
    }

    pub(crate) const fn from_index(port: u8, pin: u8) -> Self {
        Self { port, pin }
    }

    pub(crate) fn into_output(self, level: Level) {
        enable_clock(self.port);
        critical_section::with(|_| {
            let port = registers::port(self.port);
            modify_two_bit_field(port.ctl(), self.pin, MODE_OUTPUT);
            modify_two_bit_field(port.pud(), self.pin, Pull::None.field());
            modify_output_type(port, self.pin, OutputType::PushPull);
            modify_two_bit_field(port.ospd(), self.pin, Speed::High.field());
        });
        match level {
            Level::High => self.set_high(),
            Level::Low => self.set_low(),
        }
    }

    pub(crate) fn into_analog(self) {
        enable_clock(self.port);
        critical_section::with(|_| {
            let port = registers::port(self.port);
            modify_two_bit_field(port.ctl(), self.pin, MODE_ANALOG);
            modify_two_bit_field(port.pud(), self.pin, Pull::None.field());
        });
    }

    #[inline(always)]
    pub(crate) fn set_high(self) {
        write_bits(registers::port(self.port).bop(), self.mask());
    }

    #[inline(always)]
    pub(crate) fn set_low(self) {
        write_bits(registers::port(self.port).bc(), self.mask());
    }

    const fn mask(self) -> u32 {
        1 << self.pin
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
        assert_eq!(Line::new('C', 7), Line::from_index(2, 7));
        assert_eq!(Line::new('A', 15).mask(), 0x8000);
    }

    #[test]
    fn levels_set_the_high_pins_and_reset_the_low_ones() {
        let levels = Levels::new(
            0,
            [
                (8, Level::High),
                (9, Level::Low),
                (10, Level::High),
                (11, Level::Low),
            ],
        );
        assert_eq!(levels.set_reset, 0x0500 | 0x0A00 << 16);
    }
}
