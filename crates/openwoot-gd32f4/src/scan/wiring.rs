use super::SampleTime;
use crate::{gpio::Line, pac};

pub(super) const ADDRESS_LINES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalogInput {
    pub line: Line,
    pub channel: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wiring<const INPUTS: usize> {
    pub address_port: char,
    pub address_pins: [u8; ADDRESS_LINES],
    pub enable: [Line; INPUTS],
    pub inputs: [AnalogInput; INPUTS],
    pub sample_time: SampleTime,
}

#[derive(Debug)]
pub struct Peripherals {
    pub adc: pac::Adc0,
    pub adc_common: pac::AdcCommon,
    pub timer: pac::Timer1,
}
