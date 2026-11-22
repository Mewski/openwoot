#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    reason = "the helpers build fixed test tables and parse the golden data, and fail the test on a malformed line"
)]

mod effects;
mod renderer;
mod task;

use openwoot::lighting::{FramePage, Gains, Geometry, Key, Led, LedMap, LedPosition, Variant};

const ROWS: usize = 6;
const COLUMNS: usize = 17;
const PAGES: usize = 9;
const LENGTH: usize = 36;
const LEDS_PER_PAGE: usize = LENGTH / 3;
const LED_COUNT: usize = ROWS * COLUMNS + 1;
const HOLES: [(usize, usize); 14] = [
    (0, 9),
    (1, 13),
    (2, 13),
    (3, 12),
    (3, 13),
    (3, 15),
    (3, 16),
    (4, 1),
    (4, 12),
    (4, 13),
    (4, 16),
    (5, 3),
    (5, 9),
    (5, 13),
];

static MAP: LedMap<PAGES> = LedMap {
    positions: &POSITIONS,
    pages: pages(),
    gains: Gains::FULL,
};

static VARIANT: Variant<ROWS, COLUMNS> = Variant {
    leds: leds(),
    spacebar: Key::new(5, 6),
    spacebar_fill: &[4, 5, 7, 8],
};

const GEOMETRY: Geometry = Geometry {
    first_key_row: 0,
    level_meter_segments: 10,
};

static POSITIONS: [Option<LedPosition>; LED_COUNT] = positions();

const fn positions() -> [Option<LedPosition>; LED_COUNT] {
    let mut positions = [None; LED_COUNT];
    let mut led = 1;
    while led < LED_COUNT {
        let index = led - 1;
        positions[led] = Some(LedPosition::new(
            (index / LEDS_PER_PAGE) as u8,
            (index % LEDS_PER_PAGE * 3) as u8,
        ));
        led += 1;
    }
    positions
}

const fn pages() -> [FramePage; PAGES] {
    let mut pages = [FramePage::new(0, 0, LENGTH as u8); PAGES];
    let mut page = 0;
    while page < PAGES {
        let first = page * LENGTH;
        pages[page] = FramePage::new((first / 180) as u8, (first % 180) as u8, LENGTH as u8);
        page += 1;
    }
    pages
}

const fn leds() -> [[Option<Led>; COLUMNS]; ROWS] {
    let mut leds = [[None; COLUMNS]; ROWS];
    let mut row = 0;
    while row < ROWS {
        let mut column = 0;
        while column < COLUMNS {
            if !is_hole(row, column) {
                leds[row][column] = Some(Led((row * COLUMNS + column + 1) as u8));
            }
            column += 1;
        }
        row += 1;
    }
    leds
}

const fn is_hole(row: usize, column: usize) -> bool {
    let mut index = 0;
    while index < HOLES.len() {
        if HOLES[index].0 == row && HOLES[index].1 == column {
            return true;
        }
        index += 1;
    }
    false
}

const fn base_color(row: usize, column: usize) -> u16 {
    ((((row * 37 + column * 11) & 0x1F) << 11)
        | (((row * 13 + column * 29) & 0x3F) << 5)
        | ((row * 7 + column * 3) & 0x1F)) as u16
}
