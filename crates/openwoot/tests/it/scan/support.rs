use openwoot::{
    keys::Key,
    scan::{Map, Outcome, Position, Scanner, Sensing, Travel},
};

use super::simulated::{Bench, Simulated};

pub(crate) const KEYS: usize = 84;
pub(crate) const SETTLING_CYCLES: u32 = 4;
pub(crate) const PRESSED: Position = Position::new(1, 5);
const MAP: Map<6, 15> = Map::from_codes([
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
const ORDER: [u8; 6] = [0, 1, 2, 3, 4, 5];
const MAX_CONVERSIONS: usize = 100_000;

pub(crate) type Bench80 = Scanner<Bench, 6, 15>;

pub(crate) fn wobble(conversion: u32) -> i16 {
    (conversion % 3) as i16
}

pub(crate) fn scanner() -> Bench80 {
    Scanner::new(Simulated::default(), MAP, ORDER)
}

pub(crate) fn calibrate<S: Sensing, const INPUTS: usize, const ADDRESSES: usize>(
    scanner: &mut Scanner<S, INPUTS, ADDRESSES>,
    travel: &Travel<INPUTS, ADDRESSES>,
) -> usize {
    scanner.start_sweep();
    run(scanner, travel)
}

pub(crate) fn cycle<S: Sensing, const INPUTS: usize, const ADDRESSES: usize>(
    scanner: &mut Scanner<S, INPUTS, ADDRESSES>,
    travel: &Travel<INPUTS, ADDRESSES>,
    now_ms: u32,
) -> usize {
    scanner.prepare(now_ms);
    run(scanner, travel)
}

pub(crate) fn settled(scanner: &mut Bench80, travel: &Travel<6, 15>) -> u32 {
    calibrate(scanner, travel);
    for now_ms in 0..=SETTLING_CYCLES {
        cycle(scanner, travel, now_ms);
    }
    scanner.converter_mut().clear();
    SETTLING_CYCLES + 1
}

pub(crate) fn event_travels(scanner: &Bench80) -> Vec<(Key, u16)> {
    scanner
        .events()
        .iter()
        .map(|event| (event.key, event.travel))
        .collect()
}

fn run<S: Sensing, const INPUTS: usize, const ADDRESSES: usize>(
    scanner: &mut Scanner<S, INPUTS, ADDRESSES>,
    travel: &Travel<INPUTS, ADDRESSES>,
) -> usize {
    let finished =
        (1..=MAX_CONVERSIONS).find(|_| scanner.on_conversion(travel) == Outcome::Finished);
    assert!(finished.is_some(), "the scan never finished");
    finished.unwrap_or_default()
}
