use core::{convert::Infallible, marker::PhantomData};

use embedded_hal::digital::{ErrorType, InputPin, OutputPin, StatefulOutputPin};

use crate::{
    pac, rcu,
    register::{modify_bits, read_bits, set_bits, write_bits},
};

const PORT_STRIDE: usize = 0x400;
const RESET_SHIFT: u8 = 16;
const INPUT_ANALOG: u32 = 0b00 << 2;
const INPUT_FLOATING: u32 = 0b01 << 2;
const INPUT_PULL: u32 = 0b10 << 2;
const OUTPUT_PUSH_PULL: u32 = 0b00 << 2;
const OUTPUT_OPEN_DRAIN: u32 = 0b01 << 2;
const ALTERNATE_PUSH_PULL: u32 = 0b10 << 2;
const ALTERNATE_OPEN_DRAIN: u32 = 0b11 << 2;

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
    const fn mode(self) -> u32 {
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

    const fn new() -> Self {
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
                    rcu::with_registers(|rcu| set_bits(rcu.apb2en(), 1 << $enable));
                    Self::split()
                }
            }
        )*
    };
}

ports! {
    Gpioa: 'A', 2;
    Gpiob: 'B', 3;
    Gpioc: 'C', 4;
    Gpiod: 'D', 5;
    Gpioe: 'E', 6;
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

fn write_level<const PORT: char, const INDEX: u8>(level: Level) {
    let bits = match level {
        Level::High => 1 << INDEX,
        Level::Low => 1 << (INDEX + RESET_SHIFT),
    };
    write_bits(registers::<PORT>().bop(), bits);
}

fn configure<const PORT: char, const INDEX: u8>(control: u32, speed: Option<Speed>) {
    let nibble = control | speed.map_or(0, Speed::mode);
    let maximum = u32::from(speed == Some(Speed::Maximum));
    critical_section::with(|_| {
        let port = registers::<PORT>();
        modify_bits(port.spd(), |bits| (bits & !(1 << INDEX)) | maximum << INDEX);
        if INDEX < 8 {
            modify_bits(port.ctl0(), |bits| nibble_field(bits, INDEX, nibble));
        } else {
            modify_bits(port.ctl1(), |bits| nibble_field(bits, INDEX - 8, nibble));
        }
    });
}

const fn nibble_field(register: u32, index: u8, nibble: u32) -> u32 {
    let shift = index as u32 * 4;
    (register & !(0b1111 << shift)) | (nibble << shift)
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
    fn nibbles_land_on_their_pin() {
        assert_eq!(nibble_field(0x4444_4444, 7, 0x3), 0x3444_4444);
        assert_eq!(nibble_field(0x4444_4444, 0, 0xF), 0x4444_444F);
    }

    #[test]
    fn ports_lie_one_kilobyte_apart() {
        assert_eq!(port_offset('A'), 0);
        assert_eq!(port_offset('C'), 0x800);
    }

    #[test]
    fn levels_are_read_from_the_pin_bit() {
        assert_eq!(level_of(0x8000, 15), Level::High);
        assert_eq!(level_of(0x7FFF, 15), Level::Low);
    }
}
