use hal::scan::{AnalogInput, Line, Oversampling, SampleTime, Wiring};
use openwoot::{
    analog::StdDev,
    scan::{Map, MeasuredReference, Shared},
};

use crate::hal;

pub const INPUTS: usize = 4;
pub const ADDRESSES: usize = 16;
pub const PARALLEL_ORDER: [u8; INPUTS] = [0, 1, 2, 3];
pub const WIRING: Wiring<INPUTS> = Wiring {
    address_port: 'B',
    address_pins: [0, 1, 10, 11],
    enable: [
        Line::new('B', 8),
        Line::new('B', 9),
        Line::new('C', 13),
        Line::new('C', 14),
    ],
    inputs: [
        AnalogInput {
            line: Line::new('A', 0),
            channel: 0,
        },
        AnalogInput {
            line: Line::new('A', 1),
            channel: 1,
        },
        AnalogInput {
            line: Line::new('A', 2),
            channel: 2,
        },
        AnalogInput {
            line: Line::new('A', 3),
            channel: 3,
        },
    ],
    reference: AnalogInput {
        line: Line::new('A', 4),
        channel: 4,
    },
    sample_time: SampleTime::HalfCycles3,
    oversampling: Oversampling::Times8,
};

pub const ANSI: Map<INPUTS, ADDRESSES> = Map::from_codes([
    [
        0x82, 0xA2, 0x23, 0x22, 0x21, 0x42, 0x41, 0x62, 0x61, 0xFF, 0x80, 0x60, 0x40, 0x20, 0xA0,
        0xA1,
    ],
    [
        0x85, 0xA6, 0x86, 0x65, 0x46, 0x45, 0x27, 0x26, 0x64, 0x44, 0x25, 0x24, 0x43, 0x63, 0x83,
        0x84,
    ],
    [
        0x89, 0xAA, 0x8A, 0x4A, 0x2A, 0x29, 0x49, 0x69, 0x68, 0x48, 0x28, 0x47, 0x67, 0x66, 0x87,
        0x88,
    ],
    [
        0xAD, 0xAC, 0x8D, 0x6D, 0x4D, 0x2D, 0x2C, 0x4C, 0x2B, 0x4B, 0x6A, 0xAB, 0x8B, 0x6B, 0xFF,
        0xFF,
    ],
]);

pub const ISO: Map<INPUTS, ADDRESSES> = Map::from_codes([
    [
        0x82, 0xA2, 0x23, 0x22, 0x21, 0x42, 0x41, 0x62, 0x61, 0x81, 0x80, 0x60, 0x40, 0x20, 0xA0,
        0xA1,
    ],
    [
        0x85, 0xA6, 0x86, 0x65, 0x46, 0x45, 0x27, 0x26, 0x64, 0x44, 0x25, 0x24, 0x43, 0x63, 0x83,
        0x84,
    ],
    [
        0x89, 0xAA, 0x8A, 0x4A, 0x2A, 0x29, 0x49, 0x69, 0x68, 0x48, 0x28, 0x47, 0x67, 0x66, 0x87,
        0x88,
    ],
    [
        0xAD, 0xAC, 0x8D, 0x6D, 0x6C, 0x2D, 0x2C, 0x4C, 0x2B, 0x4B, 0x6A, 0xAB, 0x8B, 0x6B, 0xFF,
        0xFF,
    ],
]);

pub const SENSOR_MAPS: [Map<INPUTS, ADDRESSES>; 2] = [ANSI, ISO];

pub static SCAN: Shared<Sensing, INPUTS, ADDRESSES> = Shared::new();

pub type Converter = hal::scan::Converter<INPUTS>;
pub type Scanner = openwoot::scan::Scanner<Sensing, INPUTS, ADDRESSES>;

#[derive(Debug)]
pub struct Sensing;

impl openwoot::scan::Sensing for Sensing {
    type Converter = Converter;
    type Noise = StdDev;
    type Recheck = ();
    type Reference = MeasuredReference<2482>;
}

#[derive(Debug)]
pub struct ScanInterrupt;

impl hal::scan::Conversion for ScanInterrupt {
    fn on_conversion() {
        SCAN.on_conversion();
    }
}

pub fn select_layout(variant: u8) {
    if let Some(map) = SENSOR_MAPS.get(usize::from(variant)) {
        SCAN.with(|scanner| scanner.set_map(*map));
    }
}

pub(crate) fn install(converter: Converter) {
    if !SCAN.install(Scanner::new(converter, ANSI, PARALLEL_ORDER)) {
        hal::system::reset();
    }
}

#[cfg(test)]
mod tests {
    use openwoot::scan::{MatrixKey, Position};

    use super::*;
    use crate::{KEY_COLUMNS, KEY_ROWS};

    #[test]
    fn every_layout_has_its_keys_once_and_inside_the_matrix() {
        for (map, keys) in SENSOR_MAPS.iter().zip([61, 62]) {
            assert_eq!(map.len(), keys);
            for (position, key) in map.iter() {
                assert!(usize::from(key.row) < KEY_ROWS && usize::from(key.column) < KEY_COLUMNS);
                assert_eq!(map.position(key), Some(position));
            }
        }
    }

    #[test]
    fn every_input_carries_keys_so_the_single_input_scan_never_skips_one() {
        for map in SENSOR_MAPS {
            for input in 0..u8::try_from(INPUTS).unwrap() {
                assert!(map.input_has_keys(input));
            }
        }
    }

    #[test]
    fn the_layouts_differ_where_the_c_tables_differ() {
        assert_eq!(
            ANSI.key(Position::new(0, 0)),
            Some(MatrixKey { row: 4, column: 2 })
        );
        assert_eq!(ANSI.key(Position::new(0, 9)), None);
        let addresses = u8::try_from(ADDRESSES).unwrap();
        let differences = (0..u8::try_from(INPUTS).unwrap())
            .flat_map(|input| (0..addresses).map(move |address| Position::new(input, address)))
            .filter(|&position| ANSI.key(position) != ISO.key(position))
            .count();
        assert!(differences > 0);
    }
}
