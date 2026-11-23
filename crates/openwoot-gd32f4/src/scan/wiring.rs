use super::{SampleTime, adc::MAX_CHANNEL};
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

impl<const INPUTS: usize> Wiring<INPUTS> {
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        let mut pins: &[u8] = &self.address_pins;
        while let [pin, rest @ ..] = pins {
            if !Line::exists(self.address_port, *pin) {
                return false;
            }
            pins = rest;
        }
        let mut inputs: &[AnalogInput] = &self.inputs;
        while let [input, rest @ ..] = inputs {
            if input.channel > MAX_CHANNEL {
                return false;
            }
            inputs = rest;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIRING: Wiring<2> = Wiring {
        address_port: 'A',
        address_pins: [8, 9, 10, 11],
        enable: [Line::new('B', 14), Line::new('B', 15)],
        inputs: [
            AnalogInput {
                line: Line::new('A', 0),
                channel: 0,
            },
            AnalogInput {
                line: Line::new('C', 5),
                channel: 15,
            },
        ],
        sample_time: SampleTime::Cycles3,
    };

    const _: () = assert!(WIRING.is_valid());

    #[test]
    fn a_wiring_of_existing_pins_and_channels_is_valid() {
        assert!(WIRING.is_valid());
        let last = Wiring {
            address_port: 'I',
            address_pins: [12, 13, 14, 15],
            inputs: WIRING.inputs.map(|input| AnalogInput {
                channel: MAX_CHANNEL,
                ..input
            }),
            ..WIRING
        };
        assert!(last.is_valid());
    }

    #[test]
    fn an_address_line_outside_the_ports_and_pins_of_the_chip_is_rejected() {
        for address_port in ['J', 'O', 'Z', '@', 'a'] {
            let wiring = Wiring {
                address_port,
                ..WIRING
            };
            assert!(!wiring.is_valid(), "port {address_port}");
        }
        for address_pins in [[16, 9, 10, 11], [8, 9, 10, 16], [8, 9, 255, 11]] {
            let wiring = Wiring {
                address_pins,
                ..WIRING
            };
            assert!(!wiring.is_valid(), "pins {address_pins:?}");
        }
    }

    #[test]
    fn a_converter_channel_beyond_the_last_one_is_rejected() {
        for channel in [MAX_CHANNEL + 1, 31, 32, 255] {
            let mut inputs = WIRING.inputs;
            inputs[1].channel = channel;
            let wiring = Wiring { inputs, ..WIRING };
            assert!(!wiring.is_valid(), "channel {channel}");
        }
    }
}
