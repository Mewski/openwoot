use core::{convert::Infallible, marker::PhantomData};

use embedded_hal::digital::{ErrorType, InputPin, OutputPin, StatefulOutputPin};

use crate::{
    pac, rcu,
    register::{modify_bits, read_bits, set_bits, write_bits},
};

const PORT_STRIDE: usize = 0x400;
const MODE_INPUT: u32 = 0b00;
const MODE_OUTPUT: u32 = 0b01;
const MODE_ALTERNATE: u32 = 0b10;
const MODE_ANALOG: u32 = 0b11;
const RESET_SHIFT: u8 = 16;

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
    const fn field(self) -> u32 {
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
    const fn field(self) -> u32 {
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
            let port = registers::<PORT>();
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
            let port = registers::<PORT>();
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
            let port = registers::<PORT>();
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
            let port = registers::<PORT>();
            modify_two_bit_field(port.pud(), INDEX, Pull::None.field());
            modify_two_bit_field(port.ctl(), INDEX, MODE_ANALOG);
        });
        Pin::new()
    }

    const fn new() -> Self {
        const { assert!(INDEX < 16) };
        Self { mode: PhantomData }
    }
}

impl<const PORT: char, const INDEX: u8> Pin<PORT, INDEX, Output> {
    pub fn set_level(&mut self, level: Level) {
        let bits = match level {
            Level::High => 1 << INDEX,
            Level::Low => 1 << (INDEX + RESET_SHIFT),
        };
        write_bits(registers::<PORT>().bop(), bits);
    }

    #[must_use]
    pub fn output_level(&self) -> Level {
        level_of(read_bits(registers::<PORT>().octl()), INDEX)
    }
}

impl<const PORT: char, const INDEX: u8> Pin<PORT, INDEX, Input> {
    #[must_use]
    pub fn level(&self) -> Level {
        level_of(read_bits(registers::<PORT>().istat()), INDEX)
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

macro_rules! ports {
    ($($port:ident: $letter:literal, $enable:literal;)*) => {
        $(
            impl Port<$letter> {
                #[must_use]
                pub fn new(port: pac::$port) -> Self {
                    let _ = port;
                    rcu::with_registers(|rcu| set_bits(rcu.ahb1en(), 1 << $enable));
                    Self::split()
                }
            }
        )*
    };
}

ports! {
    Gpioa: 'A', 0;
    Gpiob: 'B', 1;
    Gpioc: 'C', 2;
    Gpiod: 'D', 3;
    Gpioe: 'E', 4;
}

#[derive(Debug)]
pub struct Port<const PORT: char> {
    pub pin0: Pin<PORT, 0, Unconfigured>,
    pub pin1: Pin<PORT, 1, Unconfigured>,
    pub pin2: Pin<PORT, 2, Unconfigured>,
    pub pin3: Pin<PORT, 3, Unconfigured>,
    pub pin4: Pin<PORT, 4, Unconfigured>,
    pub pin5: Pin<PORT, 5, Unconfigured>,
    pub pin6: Pin<PORT, 6, Unconfigured>,
    pub pin7: Pin<PORT, 7, Unconfigured>,
    pub pin8: Pin<PORT, 8, Unconfigured>,
    pub pin9: Pin<PORT, 9, Unconfigured>,
    pub pin10: Pin<PORT, 10, Unconfigured>,
    pub pin11: Pin<PORT, 11, Unconfigured>,
    pub pin12: Pin<PORT, 12, Unconfigured>,
    pub pin13: Pin<PORT, 13, Unconfigured>,
    pub pin14: Pin<PORT, 14, Unconfigured>,
    pub pin15: Pin<PORT, 15, Unconfigured>,
}

impl<const PORT: char> Port<PORT> {
    const fn split() -> Self {
        Self {
            pin0: Pin::new(),
            pin1: Pin::new(),
            pin2: Pin::new(),
            pin3: Pin::new(),
            pin4: Pin::new(),
            pin5: Pin::new(),
            pin6: Pin::new(),
            pin7: Pin::new(),
            pin8: Pin::new(),
            pin9: Pin::new(),
            pin10: Pin::new(),
            pin11: Pin::new(),
            pin12: Pin::new(),
            pin13: Pin::new(),
            pin14: Pin::new(),
            pin15: Pin::new(),
        }
    }
}

fn registers<const PORT: char>() -> &'static pac::gpioa::RegisterBlock {
    let address = pac::Gpioa::PTR as usize + port_offset(PORT);
    // SAFETY: every port of the family has this register block at `GPIOA + n * 0x400`, and a
    // `Pin` exists only for a port whose `Port` was built from its PAC peripheral. Registers that
    // several pins share are changed only inside critical sections; output levels are written
    // through the set/reset register, which changes only the pin's own bit.
    unsafe { &*(address as *const pac::gpioa::RegisterBlock) }
}

const fn port_offset(port: char) -> usize {
    (port as usize - 'A' as usize) * PORT_STRIDE
}

fn modify_two_bit_field<Spec>(register: &pac::generic::Reg<Spec>, index: u8, value: u32)
where
    Spec: pac::generic::RegisterSpec<Ux = u32> + pac::generic::Readable + pac::generic::Writable,
{
    modify_bits(register, |bits| two_bit_field(bits, index, value));
}

fn modify_output_type(port: &pac::gpioa::RegisterBlock, index: u8, output_type: OutputType) {
    let open_drain = u32::from(output_type == OutputType::OpenDrain);
    modify_bits(port.omode(), |bits| {
        (bits & !(1 << index)) | open_drain << index
    });
}

const fn two_bit_field(register: u32, index: u8, value: u32) -> u32 {
    let shift = index as u32 * 2;
    (register & !(0b11 << shift)) | (value << shift)
}

const fn alternate_field(register: u32, index: u8, function: u8) -> u32 {
    let shift = index as u32 * 4;
    (register & !(0b1111 << shift)) | ((function as u32) << shift)
}

const fn level_of(register: u32, index: u8) -> Level {
    if register & (1 << index) == 0 {
        Level::Low
    } else {
        Level::High
    }
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
        assert_eq!(two_bit_field(0xFFFF_FFFF, 5, MODE_OUTPUT), 0xFFFF_F7FF);
        assert_eq!(two_bit_field(0, 15, MODE_ANALOG), 0xC000_0000);
        assert_eq!(alternate_field(0xFFFF_FFFF, 7, 10), 0xAFFF_FFFF);
        assert_eq!(alternate_field(0, 2, 4), 0x0000_0400);
    }

    #[test]
    fn ports_lie_one_kilobyte_apart() {
        assert_eq!(port_offset('A'), 0);
        assert_eq!(port_offset('D'), 0xC00);
    }

    #[test]
    fn levels_are_read_from_the_pin_bit() {
        assert_eq!(level_of(0x0400, 10), Level::High);
        assert_eq!(level_of(0xFBFF, 10), Level::Low);
    }
}
