use core::{convert::Infallible, marker::PhantomData};

use embedded_hal::digital::{ErrorType, InputPin, OutputPin, StatefulOutputPin};

use super::registers::{
    self, MODE_ALTERNATE, MODE_ANALOG, MODE_INPUT, MODE_OUTPUT, alternate_field, level_of,
    modify_output_type, modify_two_bit_field, port_index, set_reset,
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
pub struct Alternate<const FUNCTION: u8>;

#[derive(Debug)]
pub struct Analog;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pull {
    None,
    Up,
    Down,
}

impl Pull {
    pub(super) const fn field(self) -> u32 {
        match self {
            Self::None => 0b00,
            Self::Up => 0b01,
            Self::Down => 0b10,
        }
    }
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
    pub(super) const fn field(self) -> u32 {
        match self {
            Self::Low => 0b00,
            Self::Medium => 0b01,
            Self::High => 0b10,
            Self::Maximum => 0b11,
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
        critical_section::with(|_| {
            let port = port::<PORT>();
            modify_two_bit_field(port.pud(), INDEX, pull.field());
            modify_two_bit_field(port.ctl(), INDEX, MODE_INPUT);
        });
        Pin::new()
    }

    #[must_use]
    pub fn into_output(
        self,
        output_type: OutputType,
        speed: Speed,
        level: Level,
    ) -> Pin<PORT, INDEX, Output> {
        let mut pin = Pin::new();
        pin.set_level(level);
        critical_section::with(|_| {
            let port = port::<PORT>();
            modify_output_type(port, INDEX, output_type);
            modify_two_bit_field(port.ospd(), INDEX, speed.field());
            modify_two_bit_field(port.pud(), INDEX, Pull::None.field());
            modify_two_bit_field(port.ctl(), INDEX, MODE_OUTPUT);
        });
        pin
    }

    #[must_use]
    pub fn into_alternate<const FUNCTION: u8>(
        self,
        output_type: OutputType,
        speed: Speed,
        pull: Pull,
    ) -> Pin<PORT, INDEX, Alternate<FUNCTION>> {
        const { assert!(FUNCTION < 16) };
        critical_section::with(|_| {
            let port = port::<PORT>();
            if INDEX < 8 {
                modify_bits(port.afsel0(), |bits| alternate_field(bits, INDEX, FUNCTION));
            } else {
                modify_bits(port.afsel1(), |bits| {
                    alternate_field(bits, INDEX - 8, FUNCTION)
                });
            }
            modify_output_type(port, INDEX, output_type);
            modify_two_bit_field(port.ospd(), INDEX, speed.field());
            modify_two_bit_field(port.pud(), INDEX, pull.field());
            modify_two_bit_field(port.ctl(), INDEX, MODE_ALTERNATE);
        });
        Pin::new()
    }

    #[must_use]
    pub fn into_analog(self) -> Pin<PORT, INDEX, Analog> {
        critical_section::with(|_| {
            let port = port::<PORT>();
            modify_two_bit_field(port.pud(), INDEX, Pull::None.field());
            modify_two_bit_field(port.ctl(), INDEX, MODE_ANALOG);
        });
        Pin::new()
    }

    pub(super) const fn new() -> Self {
        const { assert!(INDEX < 16) };
        Self { mode: PhantomData }
    }
}

impl<const PORT: char, const INDEX: u8> Pin<PORT, INDEX, Output> {
    pub fn set_level(&mut self, level: Level) {
        write_bits(port::<PORT>().bop(), set_reset(INDEX, level));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_carry_the_reference_manual_encodings() {
        assert_eq!(Speed::High.field(), 0b10);
        assert_eq!(Speed::Maximum.field(), 0b11);
        assert_eq!(Pull::Up.field(), 0b01);
        assert_eq!(Pull::Down.field(), 0b10);
    }
}
