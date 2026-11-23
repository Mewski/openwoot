use super::{Oversampling, SampleTime};
use crate::{gpio::Line, pac};

pub(super) const ADDRESS_LINES: usize = 4;

const LAST_CHANNEL: u8 = 15;

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
    pub reference: AnalogInput,
    pub sample_time: SampleTime,
    pub oversampling: Oversampling,
}

impl<const INPUTS: usize> Wiring<INPUTS> {
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        pins_exist(self.address_port, &self.address_pins)
            && channels_exist(&self.inputs)
            && self.reference.channel <= LAST_CHANNEL
    }
}

const fn pins_exist(port: char, mut pins: &[u8]) -> bool {
    while let [pin, rest @ ..] = pins {
        if !Line::exists(port, *pin) {
            return false;
        }
        pins = rest;
    }
    true
}

const fn channels_exist(mut inputs: &[AnalogInput]) -> bool {
    while let [input, rest @ ..] = inputs {
        if input.channel > LAST_CHANNEL {
            return false;
        }
        inputs = rest;
    }
    true
}

#[derive(Debug)]
pub struct Peripherals {
    pub adc: pac::Adc0,
    pub reference_adc: pac::Adc1,
    pub timer: pac::Timer1,
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIRING: Wiring<2> = Wiring {
        address_port: 'B',
        address_pins: [0, 1, 10, 11],
        enable: [Line::new('B', 8), Line::new('C', 13)],
        inputs: [
            AnalogInput {
                line: Line::new('A', 0),
                channel: 0,
            },
            AnalogInput {
                line: Line::new('A', 1),
                channel: 15,
            },
        ],
        reference: AnalogInput {
            line: Line::new('A', 4),
            channel: 4,
        },
        sample_time: SampleTime::HalfCycles3,
        oversampling: Oversampling::Times8,
    };

    const _: () = assert!(WIRING.is_valid());

    #[test]
    fn address_lines_outside_ports_a_to_g_or_pins_0_to_15_are_refused() {
        assert!(WIRING.is_valid());
        for address_port in ['H', 'Z', '@', 'b', '\u{87}'] {
            assert!(
                !Wiring {
                    address_port,
                    ..WIRING
                }
                .is_valid(),
                "{address_port}"
            );
        }
        assert!(
            !Wiring {
                address_pins: [0, 1, 10, 16],
                ..WIRING
            }
            .is_valid()
        );
    }

    #[test]
    fn converter_channels_without_a_pin_are_refused() {
        let internal = AnalogInput {
            line: Line::new('A', 4),
            channel: 16,
        };
        assert!(
            !Wiring {
                reference: internal,
                ..WIRING
            }
            .is_valid()
        );
        assert!(
            !Wiring {
                inputs: [internal, internal],
                ..WIRING
            }
            .is_valid()
        );
    }
}
