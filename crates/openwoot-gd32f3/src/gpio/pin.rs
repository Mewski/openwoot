use core::{convert::Infallible, marker::PhantomData};

use embedded_hal::digital::{ErrorType, InputPin, OutputPin, StatefulOutputPin};

use super::registers::{
    self, ALTERNATE_OPEN_DRAIN, ALTERNATE_PUSH_PULL, INPUT_ANALOG, INPUT_FLOATING, INPUT_PULL,
    OUTPUT_OPEN_DRAIN, OUTPUT_PUSH_PULL, level_of, port_index, set_reset,
};
use crate::{
    pac,
    register::{modify_bits, read_bits, write_bits},
};

#[derive(Debug)]
pub struct Unconfigured;

#[derive(Debug)]
pub struct Input;

#[derive(Debug)]
pub struct Output;

#[derive(Debug)]
pub struct Alternate;

#[derive(Debug)]
pub struct Analog;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pull {
    None,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputType {
    PushPull,
    OpenDrain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed {
    Low,
    Medium,
    High,
    Maximum,
}

impl Speed {
    pub(super) const fn mode(self) -> u32 {
        match self {
            Self::Medium => 0b01,
            Self::Low => 0b10,
            Self::High | Self::Maximum => 0b11,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Low,
    High,
}

#[derive(Debug)]
pub struct Pin<const PORT: char, const INDEX: u8, Mode> {
    mode: PhantomData<Mode>,
}

impl<const PORT: char, const INDEX: u8, Mode> Pin<PORT, INDEX, Mode> {
    #[must_use]
    pub fn into_input(self, pull: Pull) -> Pin<PORT, INDEX, Input> {
        let control = match pull {
            Pull::None => INPUT_FLOATING,
            Pull::Up => {
                write_level::<PORT, INDEX>(Level::High);
                INPUT_PULL
            }
            Pull::Down => {
                write_level::<PORT, INDEX>(Level::Low);
                INPUT_PULL
            }
        };
        configure::<PORT, INDEX>(control, None);
        Pin::new()
    }

    #[must_use]
    pub fn into_output(
        self,
        output_type: OutputType,
        speed: Speed,
        level: Level,
    ) -> Pin<PORT, INDEX, Output> {
        write_level::<PORT, INDEX>(level);
        let control = match output_type {
            OutputType::PushPull => OUTPUT_PUSH_PULL,
            OutputType::OpenDrain => OUTPUT_OPEN_DRAIN,
        };
        configure::<PORT, INDEX>(control, Some(speed));
        Pin::new()
    }

    #[must_use]
    pub fn into_alternate(
        self,
        output_type: OutputType,
        speed: Speed,
    ) -> Pin<PORT, INDEX, Alternate> {
        let control = match output_type {
            OutputType::PushPull => ALTERNATE_PUSH_PULL,
            OutputType::OpenDrain => ALTERNATE_OPEN_DRAIN,
        };
        configure::<PORT, INDEX>(control, Some(speed));
        Pin::new()
    }

    #[must_use]
    pub fn into_analog(self) -> Pin<PORT, INDEX, Analog> {
        configure::<PORT, INDEX>(INPUT_ANALOG, None);
        Pin::new()
    }

    pub(super) const fn new() -> Self {
        const { assert!(INDEX < 16) };
        Self { mode: PhantomData }
    }
}

impl<const PORT: char, const INDEX: u8> Pin<PORT, INDEX, Output> {
    pub fn set_level(&mut self, level: Level) {
        write_level::<PORT, INDEX>(level);
    }

    #[must_use]
    pub fn output_level(&self) -> Level {
        level_of(read_bits(port::<PORT>().octl()), INDEX)
    }
}

impl<const PORT: char, const INDEX: u8> Pin<PORT, INDEX, Input> {
    #[must_use]
    pub fn level(&self) -> Level {
        level_of(read_bits(port::<PORT>().istat()), INDEX)
    }
}

impl<const PORT: char, const INDEX: u8, Mode> ErrorType for Pin<PORT, INDEX, Mode> {
    type Error = Infallible;
}

impl<const PORT: char, const INDEX: u8> OutputPin for Pin<PORT, INDEX, Output> {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.set_level(Level::Low);
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.set_level(Level::High);
        Ok(())
    }
}

impl<const PORT: char, const INDEX: u8> StatefulOutputPin for Pin<PORT, INDEX, Output> {
    fn is_set_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.output_level() == Level::High)
    }

    fn is_set_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.output_level() == Level::Low)
    }
}

impl<const PORT: char, const INDEX: u8> InputPin for Pin<PORT, INDEX, Input> {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.level() == Level::High)
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(self.level() == Level::Low)
    }
}

fn port<const PORT: char>() -> &'static pac::gpioa::RegisterBlock {
    registers::port(port_index(PORT))
}

fn write_level<const PORT: char, const INDEX: u8>(level: Level) {
    write_bits(port::<PORT>().bop(), set_reset(INDEX, level));
}

fn configure<const PORT: char, const INDEX: u8>(control: u32, speed: Option<Speed>) {
    let nibble = control | speed.map_or(0, Speed::mode);
    critical_section::with(|_| {
        let port = port::<PORT>();
        modify_bits(port.spd(), |bits| bits & !(1 << INDEX));
        registers::set_mode(port, INDEX, nibble);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_nibbles_match_the_library_modes() {
        assert_eq!(OUTPUT_PUSH_PULL | Speed::High.mode(), 0x3);
        assert_eq!(ALTERNATE_OPEN_DRAIN | Speed::High.mode(), 0xF);
        assert_eq!(ALTERNATE_PUSH_PULL | Speed::High.mode(), 0xB);
        assert_eq!(INPUT_FLOATING, 0x4);
        assert_eq!(INPUT_PULL, 0x8);
        assert_eq!(INPUT_ANALOG, 0x0);
        assert_eq!(OUTPUT_OPEN_DRAIN | Speed::Medium.mode(), 0x5);
        assert_eq!(OUTPUT_PUSH_PULL | Speed::Low.mode(), 0x2);
    }

    #[test]
    fn the_fastest_speed_is_the_50_megahertz_mode_that_needs_no_compensation_cell() {
        assert_eq!(Speed::Maximum.mode(), Speed::High.mode());
        assert_eq!(ALTERNATE_PUSH_PULL | Speed::Maximum.mode(), 0xB);
    }
}
