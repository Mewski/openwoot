use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use hal::{
    gpio::{Level, Output, OutputType, Pin, Speed},
    i2c::{self, I2c},
    interrupt::{Binding, typelevel},
    pac,
    rcu::Clocks,
};
use openwoot::{
    lighting::{FramePage, Gains, Geometry, Key, Led, LedMap, LedPosition, Matrix, Variant},
    runtime::LightingTask,
};
use openwoot_drivers::{Is31fl3741, LedConfiguration};

use crate::hal;

pub const ROWS: usize = 6;
pub const COLUMNS: usize = 14;
pub const PAGES: usize = 7;
pub const PAGE_LENGTH: usize = 30;
pub const LED_COUNT: usize = 108;
pub const LED_DRIVER_ADDRESS: u8 = 0x30;
pub const LED_DRIVER_RUNNING: LedConfiguration = LedConfiguration::new(0x31);
pub const GEOMETRY: Geometry = Geometry {
    first_key_row: 1,
    level_meter_segments: 12,
};
pub const SPACEBAR: Key = Key::new(5, 6);

pub type Lighting = openwoot::lighting::Lighting<(), ROWS, COLUMNS, PAGES, PAGE_LENGTH>;
pub type SharedLighting = Mutex<CriticalSectionRawMutex, Lighting>;
pub type Task =
    LightingTask<'static, CriticalSectionRawMutex, (), ROWS, COLUMNS, PAGES, PAGE_LENGTH>;
pub type LedDriver = Matrix<I2c, Pin<'B', 5, Output>>;

pub fn led_driver<Mode>(
    i2c: pac::I2c0,
    clock: impl i2c::ClockPin,
    data: impl i2c::DataPin,
    enable: Pin<'B', 5, Mode>,
    irqs: impl Binding<typelevel::I2C0_EV, i2c::InterruptHandler>
    + Binding<typelevel::I2C0_ER, i2c::InterruptHandler>
    + Binding<typelevel::DMA0_Channel5, i2c::InterruptHandler>,
    clocks: &Clocks,
) -> LedDriver {
    let bus = I2c::new(i2c, clock, data, irqs, clocks);
    let enable = enable.into_output(OutputType::PushPull, Speed::High, Level::Low);
    Matrix::new(
        Is31fl3741::new(bus, enable, LED_DRIVER_ADDRESS),
        LED_DRIVER_RUNNING,
    )
}

#[must_use]
pub fn lighting(layout_variant: u8) -> Lighting {
    Lighting::new(&LEDS, variant(layout_variant), GEOMETRY, (), 0)
}

pub static LEDS: LedMap<PAGES> = LedMap {
    positions: &POSITIONS,
    pages: [
        FramePage::new(0, 0, 30),
        FramePage::new(0, 30, 30),
        FramePage::new(0, 60, 30),
        FramePage::new(0, 90, 30),
        FramePage::new(0, 120, 30),
        FramePage::new(1, 90, 27),
        FramePage::new(1, 117, 27),
    ],
    gains: Gains {
        red: 232,
        green: 255,
        blue: 255,
    },
};

pub static ANSI: Variant<ROWS, COLUMNS> = Variant {
    leds: [
        [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        ],
        [
            Some(Led(0)),
            Some(Led(1)),
            Some(Led(2)),
            Some(Led(3)),
            Some(Led(4)),
            Some(Led(5)),
            Some(Led(6)),
            Some(Led(7)),
            Some(Led(8)),
            Some(Led(9)),
            Some(Led(90)),
            Some(Led(91)),
            Some(Led(92)),
            Some(Led(107)),
        ],
        [
            Some(Led(10)),
            Some(Led(11)),
            Some(Led(12)),
            Some(Led(13)),
            Some(Led(14)),
            Some(Led(15)),
            Some(Led(16)),
            Some(Led(17)),
            Some(Led(18)),
            Some(Led(19)),
            Some(Led(93)),
            Some(Led(94)),
            Some(Led(95)),
            Some(Led(98)),
        ],
        [
            Some(Led(20)),
            Some(Led(21)),
            Some(Led(22)),
            Some(Led(23)),
            Some(Led(24)),
            Some(Led(25)),
            Some(Led(26)),
            Some(Led(27)),
            Some(Led(28)),
            Some(Led(29)),
            Some(Led(96)),
            Some(Led(97)),
            None,
            Some(Led(104)),
        ],
        [
            Some(Led(30)),
            None,
            Some(Led(32)),
            Some(Led(33)),
            Some(Led(34)),
            Some(Led(35)),
            Some(Led(36)),
            Some(Led(37)),
            Some(Led(38)),
            Some(Led(39)),
            Some(Led(99)),
            Some(Led(100)),
            None,
            Some(Led(101)),
        ],
        [
            Some(Led(40)),
            Some(Led(41)),
            Some(Led(42)),
            None,
            Some(Led(43)),
            Some(Led(44)),
            Some(Led(45)),
            Some(Led(46)),
            Some(Led(47)),
            None,
            Some(Led(48)),
            Some(Led(49)),
            Some(Led(102)),
            Some(Led(103)),
        ],
    ],
    spacebar: SPACEBAR,
    spacebar_fill: &[4, 5, 7, 8],
};

pub static ISO: Variant<ROWS, COLUMNS> = Variant {
    leds: [
        [
            None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        ],
        [
            Some(Led(0)),
            Some(Led(1)),
            Some(Led(2)),
            Some(Led(3)),
            Some(Led(4)),
            Some(Led(5)),
            Some(Led(6)),
            Some(Led(7)),
            Some(Led(8)),
            Some(Led(9)),
            Some(Led(90)),
            Some(Led(91)),
            Some(Led(92)),
            Some(Led(107)),
        ],
        [
            Some(Led(10)),
            Some(Led(11)),
            Some(Led(12)),
            Some(Led(13)),
            Some(Led(14)),
            Some(Led(15)),
            Some(Led(16)),
            Some(Led(17)),
            Some(Led(18)),
            Some(Led(19)),
            Some(Led(93)),
            Some(Led(94)),
            Some(Led(95)),
            None,
        ],
        [
            Some(Led(20)),
            Some(Led(21)),
            Some(Led(22)),
            Some(Led(23)),
            Some(Led(24)),
            Some(Led(25)),
            Some(Led(26)),
            Some(Led(27)),
            Some(Led(28)),
            Some(Led(29)),
            Some(Led(96)),
            Some(Led(97)),
            Some(Led(98)),
            Some(Led(104)),
        ],
        [
            Some(Led(30)),
            Some(Led(31)),
            Some(Led(32)),
            Some(Led(33)),
            Some(Led(34)),
            Some(Led(35)),
            Some(Led(36)),
            Some(Led(37)),
            Some(Led(38)),
            Some(Led(39)),
            Some(Led(99)),
            Some(Led(100)),
            None,
            Some(Led(101)),
        ],
        [
            Some(Led(40)),
            Some(Led(41)),
            Some(Led(42)),
            None,
            Some(Led(43)),
            Some(Led(44)),
            Some(Led(45)),
            Some(Led(46)),
            Some(Led(47)),
            None,
            Some(Led(48)),
            Some(Led(49)),
            Some(Led(102)),
            Some(Led(103)),
        ],
    ],
    spacebar: SPACEBAR,
    spacebar_fill: &[4, 5, 7, 8],
};

pub static UNKNOWN: Variant<ROWS, COLUMNS> = Variant {
    leds: ANSI.leds,
    spacebar: SPACEBAR,
    spacebar_fill: &[],
};

static POSITIONS: [Option<LedPosition>; LED_COUNT] = [
    Some(LedPosition::new(0, 0)),
    Some(LedPosition::new(0, 3)),
    Some(LedPosition::new(0, 6)),
    Some(LedPosition::new(0, 9)),
    Some(LedPosition::new(0, 12)),
    Some(LedPosition::new(0, 15)),
    Some(LedPosition::new(0, 18)),
    Some(LedPosition::new(0, 21)),
    Some(LedPosition::new(0, 24)),
    Some(LedPosition::new(0, 27)),
    Some(LedPosition::new(1, 0)),
    Some(LedPosition::new(1, 3)),
    Some(LedPosition::new(1, 6)),
    Some(LedPosition::new(1, 9)),
    Some(LedPosition::new(1, 12)),
    Some(LedPosition::new(1, 15)),
    Some(LedPosition::new(1, 18)),
    Some(LedPosition::new(1, 21)),
    Some(LedPosition::new(1, 24)),
    Some(LedPosition::new(1, 27)),
    Some(LedPosition::new(2, 0)),
    Some(LedPosition::new(2, 3)),
    Some(LedPosition::new(2, 6)),
    Some(LedPosition::new(2, 9)),
    Some(LedPosition::new(2, 12)),
    Some(LedPosition::new(2, 15)),
    Some(LedPosition::new(2, 18)),
    Some(LedPosition::new(2, 21)),
    Some(LedPosition::new(2, 24)),
    Some(LedPosition::new(2, 27)),
    Some(LedPosition::new(3, 0)),
    Some(LedPosition::new(3, 3)),
    Some(LedPosition::new(3, 6)),
    Some(LedPosition::new(3, 9)),
    Some(LedPosition::new(3, 12)),
    Some(LedPosition::new(3, 15)),
    Some(LedPosition::new(3, 18)),
    Some(LedPosition::new(3, 21)),
    Some(LedPosition::new(3, 24)),
    Some(LedPosition::new(3, 27)),
    Some(LedPosition::new(4, 0)),
    Some(LedPosition::new(4, 3)),
    Some(LedPosition::new(4, 6)),
    Some(LedPosition::new(4, 9)),
    Some(LedPosition::new(4, 12)),
    Some(LedPosition::new(4, 15)),
    Some(LedPosition::new(4, 18)),
    Some(LedPosition::new(4, 21)),
    Some(LedPosition::new(4, 24)),
    Some(LedPosition::new(4, 27)),
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(LedPosition::new(5, 0)),
    Some(LedPosition::new(5, 3)),
    Some(LedPosition::new(5, 6)),
    Some(LedPosition::new(5, 9)),
    Some(LedPosition::new(5, 12)),
    Some(LedPosition::new(5, 15)),
    Some(LedPosition::new(5, 18)),
    Some(LedPosition::new(5, 21)),
    Some(LedPosition::new(5, 24)),
    Some(LedPosition::new(6, 0)),
    Some(LedPosition::new(6, 3)),
    Some(LedPosition::new(6, 6)),
    Some(LedPosition::new(6, 9)),
    Some(LedPosition::new(6, 12)),
    Some(LedPosition::new(6, 15)),
    Some(LedPosition::new(6, 18)),
    Some(LedPosition::new(6, 21)),
    Some(LedPosition::new(6, 24)),
];

#[must_use]
pub fn variant(layout_variant: u8) -> &'static Variant<ROWS, COLUMNS> {
    match layout_variant {
        0 => &ANSI,
        1 => &ISO,
        _ => &UNKNOWN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUS_HZ: u64 = 400_000;
    const BITS_PER_BYTE: u64 = 9;
    const BITS_PER_TRANSFER: u64 = 2;

    fn variants() -> [&'static Variant<ROWS, COLUMNS>; 3] {
        [&ANSI, &ISO, &UNKNOWN]
    }

    #[test]
    fn every_key_led_has_a_place_in_the_frame() {
        assert!(LEDS.fits(PAGE_LENGTH));
        for variant in variants() {
            assert!(variant.leds[0].iter().all(Option::is_none));
            for led in variant.leds.iter().flatten().flatten() {
                assert!(LEDS.position(*led).is_some(), "{led:?}");
            }
        }
        assert_eq!(LEDS.positions.iter().flatten().count(), 68);
    }

    #[test]
    fn frame_pages_stay_inside_the_driver_banks() {
        for page in LEDS.pages {
            let end = usize::from(page.first_register) + usize::from(page.length);
            assert!(end <= if page.bank == 0 { 180 } else { 171 });
        }
    }

    #[test]
    fn red_is_limited_to_232() {
        assert_eq!(LEDS.gains.red, 232);
        assert_eq!(LEDS.gains.green, 255);
    }

    #[test]
    fn layout_numbers_select_their_variant_and_unknown_ones_have_no_space_bar_fill() {
        assert!(core::ptr::eq(variant(0), &raw const ANSI));
        assert!(core::ptr::eq(variant(1), &raw const ISO));
        assert_eq!(variant(2).spacebar_fill, &[] as &[u8]);
        assert_eq!(variant(2).leds, ANSI.leds);
    }

    #[test]
    fn a_full_frame_fits_the_bus_within_one_frame_period() {
        let bits: u64 = LEDS
            .pages
            .iter()
            .map(|page| (2 + u64::from(page.length)) * BITS_PER_BYTE + BITS_PER_TRANSFER)
            .sum::<u64>()
            + 4 * (3 * BITS_PER_BYTE + BITS_PER_TRANSFER);
        let microseconds = bits * 1_000_000 / BUS_HZ;
        assert!(microseconds < u64::from(openwoot::lighting::FRAME_PERIOD_MS) * 1000);
        assert_eq!(microseconds, 5_230);
    }
}
