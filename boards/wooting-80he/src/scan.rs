use hal::{
    gpio::Line,
    scan::{AnalogInput, SampleTime, Wiring},
};
use openwoot::{
    analog::MinMax,
    scan::{Map, RecheckFrom, Shared, ZeroLevel},
};

use crate::hal;

pub const INPUTS: usize = 6;
pub const ADDRESSES: usize = 15;
pub const PARALLEL_ORDER: [u8; INPUTS] = [0, 1, 2, 3, 4, 5];
pub const WIRING: Wiring<INPUTS> = Wiring {
    address_port: 'A',
    address_pins: [8, 9, 10, 11],
    enable: [
        Line::new('B', 14),
        Line::new('B', 15),
        Line::new('C', 6),
        Line::new('C', 7),
        Line::new('C', 8),
        Line::new('C', 9),
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
            line: Line::new('C', 1),
            channel: 11,
        },
        AnalogInput {
            line: Line::new('C', 5),
            channel: 15,
        },
        AnalogInput {
            line: Line::new('C', 4),
            channel: 14,
        },
    ],
    sample_time: SampleTime::Cycles3,
};

pub const ANSI: Map<INPUTS, ADDRESSES> = Map::from_codes([
    [
        0x44, 0x04, 0x24, 0x43, 0x03, 0x23, 0x42, 0x22, 0x02, 0x41, 0x01, 0x21, 0x00, 0x20, 0xFF,
        0xFF,
    ],
    [
        0x82, 0xA2, 0x83, 0x84, 0x85, 0x64, 0x63, 0x62, 0x61, 0x40, 0x60, 0x80, 0xA0, 0xFF, 0xA1,
        0xFF,
    ],
    [
        0x0A, 0x08, 0x29, 0x48, 0x28, 0x47, 0x27, 0x46, 0x26, 0x07, 0x06, 0x05, 0x25, 0x45, 0xFF,
        0xFF,
    ],
    [
        0x88, 0x89, 0x8A, 0xAA, 0xAB, 0x69, 0x49, 0x68, 0x67, 0x66, 0x65, 0x86, 0xA6, 0x87, 0xFF,
        0xFF,
    ],
    [
        0x2E, 0x2F, 0x4F, 0x50, 0x30, 0x10, 0x0F, 0x0E, 0x0D, 0x0C, 0x0B, 0x2A, 0x2B, 0x2C, 0xFF,
        0xFF,
    ],
    [
        0xAC, 0x8E, 0xAE, 0xAF, 0xB0, 0x8F, 0x6E, 0x4E, 0x6B, 0x4C, 0x4B, 0x4A, 0x6A, 0x8B, 0xFF,
        0xFF,
    ],
]);

pub const ISO: Map<INPUTS, ADDRESSES> = Map::from_codes([
    [
        0x44, 0x04, 0x24, 0x43, 0x03, 0x23, 0x42, 0x22, 0x02, 0x41, 0x01, 0x21, 0x00, 0x20, 0xFF,
        0xFF,
    ],
    [
        0x82, 0xA2, 0x83, 0x84, 0x85, 0x64, 0x63, 0x62, 0x61, 0x40, 0x60, 0x80, 0xA0, 0x81, 0xA1,
        0xFF,
    ],
    [
        0x0A, 0x08, 0x29, 0x48, 0x28, 0x47, 0x27, 0x46, 0x26, 0x07, 0x06, 0x05, 0x25, 0x45, 0xFF,
        0xFF,
    ],
    [
        0x88, 0x89, 0x8A, 0xAA, 0xAB, 0x69, 0x49, 0x68, 0x67, 0x66, 0x65, 0x86, 0xA6, 0x87, 0xFF,
        0xFF,
    ],
    [
        0x2E, 0x2F, 0x4F, 0x50, 0x30, 0x10, 0x0F, 0x0E, 0x0D, 0x0C, 0x0B, 0x2A, 0x2B, 0x2C, 0xFF,
        0xFF,
    ],
    [
        0xAC, 0x8E, 0xAE, 0xAF, 0xB0, 0x8F, 0x6E, 0x6C, 0x6B, 0x4C, 0x4B, 0x4A, 0x6A, 0x8B, 0xFF,
        0xFF,
    ],
]);

pub const JIS: Map<INPUTS, ADDRESSES> = Map::from_codes([
    [
        0x44, 0x04, 0x24, 0x43, 0x03, 0x23, 0x42, 0x22, 0x02, 0x41, 0x01, 0x21, 0x00, 0x20, 0xFF,
        0xFF,
    ],
    [
        0x82, 0xA3, 0x83, 0x84, 0x85, 0x64, 0x63, 0x62, 0x61, 0x40, 0x60, 0x80, 0xA0, 0xA1, 0xA2,
        0xFF,
    ],
    [
        0x0A, 0x08, 0x29, 0x48, 0x28, 0x47, 0x27, 0x46, 0x26, 0x07, 0x06, 0x05, 0x25, 0x45, 0xFF,
        0xFF,
    ],
    [
        0x88, 0x89, 0x8A, 0xAA, 0xAB, 0x69, 0x49, 0x68, 0x67, 0x66, 0x65, 0x86, 0xA6, 0x87, 0xA9,
        0xFF,
    ],
    [
        0x2D, 0x2F, 0x4F, 0x50, 0x30, 0x10, 0x0F, 0x0E, 0x0D, 0x0C, 0x0B, 0x2A, 0x2B, 0x2C, 0x2E,
        0xFF,
    ],
    [
        0xAC, 0x8C, 0xAE, 0xAF, 0xB0, 0x8F, 0x6E, 0x6C, 0x6B, 0x4C, 0x4B, 0x4A, 0x6A, 0x8B, 0x8E,
        0xFF,
    ],
]);

pub const SENSOR_MAPS: [Map<INPUTS, ADDRESSES>; 3] = [ANSI, ISO, JIS];

pub static SCAN: Shared<Sensing, INPUTS, ADDRESSES> = Shared::new();

pub type Converter = hal::scan::Converter<INPUTS>;
pub type Scanner = openwoot::scan::Scanner<Sensing, INPUTS, ADDRESSES>;

#[derive(Debug)]
pub struct Sensing;

impl openwoot::scan::Sensing for Sensing {
    type Converter = Converter;
    type Noise = MinMax;
    type Recheck = RecheckFrom<3>;
    type Reference = ZeroLevel<2048>;
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

#[cfg(test)]
mod tests {
    use openwoot::{keys::Key, scan::Position};

    use super::*;
    use crate::{KEY_COLUMNS, KEY_ROWS};

    #[test]
    fn every_layout_has_its_keys_once_and_inside_the_matrix() {
        for (map, keys) in SENSOR_MAPS.iter().zip([84, 85, 88]) {
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
    fn each_layout_variant_places_its_own_keys() {
        assert_eq!(
            ANSI.key(Position::new(0, 0)),
            Some(Key { row: 2, column: 4 })
        );
        assert_eq!(ANSI.key(Position::new(1, 13)), None);
        assert_eq!(
            ISO.key(Position::new(1, 13)),
            Some(Key { row: 4, column: 1 })
        );
        assert_eq!(
            JIS.key(Position::new(4, 14)),
            Some(Key { row: 1, column: 14 })
        );
    }
}
