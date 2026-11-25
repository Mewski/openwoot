#![allow(
    clippy::inline_always,
    reason = "the level writes run inside the scan interrupt, inlined to keep it short"
)]

use super::{
    Level, OutputType, Pull, Speed,
    registers::{
        self, LAST_PORT, MODE_ALTERNATE, MODE_ANALOG, MODE_OUTPUT, alternate_field, enable_clock,
        level_of, modify_output_type, modify_two_bit_field, port_index, set_reset,
    },
};
use crate::register::{modify_bits, read_bits, write_bits};

const PINS: u8 = 16;

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
        assert!(Self::exists(port, pin), "a pin of ports A to I");
        Self::from_index(port_index(port), pin)
    }

    #[must_use]
    pub const fn exists(port: char, pin: u8) -> bool {
        port >= 'A' && port <= LAST_PORT && pin < PINS
    }

    pub(crate) const fn from_index(port: u8, pin: u8) -> Self {
        Self { port, pin }
    }

    pub(crate) fn into_output(self, level: Level) {
        self.into_driven(OutputType::PushPull, level);
    }

    pub(crate) fn into_open_drain(self, level: Level) {
        self.into_driven(OutputType::OpenDrain, level);
    }

    pub(crate) fn into_alternate_open_drain(self, function: u8) {
        enable_clock(self.port);
        critical_section::with(|_| {
            let port = registers::port(self.port);
            if self.pin < 8 {
                modify_bits(port.afsel0(), |bits| {
                    alternate_field(bits, self.pin, function)
                });
            } else {
                modify_bits(port.afsel1(), |bits| {
                    alternate_field(bits, self.pin - 8, function)
                });
            }
            modify_output_type(port, self.pin, OutputType::OpenDrain);
            modify_two_bit_field(port.ospd(), self.pin, Speed::High.field());
            modify_two_bit_field(port.pud(), self.pin, Pull::None.field());
            modify_two_bit_field(port.ctl(), self.pin, MODE_ALTERNATE);
        });
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

    pub(crate) fn level(self) -> Level {
        level_of(read_bits(registers::port(self.port).istat()), self.pin)
    }

    fn into_driven(self, output_type: OutputType, level: Level) {
        enable_clock(self.port);
        match level {
            Level::High => self.set_high(),
            Level::Low => self.set_low(),
        }
        critical_section::with(|_| {
            let port = registers::port(self.port);
            modify_output_type(port, self.pin, output_type);
            modify_two_bit_field(port.ospd(), self.pin, Speed::High.field());
            modify_two_bit_field(port.pud(), self.pin, Pull::None.field());
            modify_two_bit_field(port.ctl(), self.pin, MODE_OUTPUT);
        });
    }

    #[inline(always)]
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
    fn only_the_sixteen_pins_of_ports_a_to_i_exist() {
        assert!(Line::exists('A', 0));
        assert!(Line::exists('I', 15));
        assert!(!Line::exists('J', 0));
        assert!(!Line::exists('@', 0));
        assert!(!Line::exists('a', 0));
        assert!(!Line::exists('A', 16));
        assert!(!Line::exists('I', 255));
        assert_eq!(Line::new('I', 15), Line::from_index(8, 15));
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
