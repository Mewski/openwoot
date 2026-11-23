use openwoot::{
    analog::{NoiseState, NoiseStatistics},
    keys::Key,
    scan::{Batching, Map, Mode, Position, Scanner, Travel},
};

use super::{
    simulated::{ReferenceBench, Simulated},
    support::{SETTLING_CYCLES, calibrate, cycle, sensors},
};

fn reference_scanner(reference: u16, reading: u16) -> Scanner<ReferenceBench, 4, 16> {
    let mut codes = [[0xFF; 16]; 4];
    for (row, keys) in (1..).zip(codes.iter_mut()) {
        for (column, code) in (0..).zip(keys.iter_mut()) {
            *code = Key { row, column }.code();
        }
    }
    let mut scanner: Scanner<ReferenceBench, 4, 16> = Scanner::new(
        Simulated::default(),
        Map::from_codes(codes),
        [0, 1, 2, 3],
        sensors(),
    );
    scanner.converter_mut().reference = reference;
    scanner.converter_mut().readings = [[reading; 16]; 4];
    scanner.converter_mut().readings[2][7] = reading + 500;
    scanner
}

#[test]
fn a_measured_reference_replaces_the_zero_level() {
    let mut scanner = reference_scanner(2400, 2400);
    let travel = Travel::new();
    calibrate(&mut scanner, &travel);
    assert_eq!(scanner.reference_level(), 2400);
    assert_eq!(
        scanner
            .sensor(Position::new(2, 7))
            .unwrap()
            .single()
            .rest_level,
        500
    );
    assert_eq!(
        scanner
            .sensor(Position::new(0, 0))
            .unwrap()
            .single()
            .rest_level,
        0
    );
}

#[test]
fn a_reference_that_reads_zero_falls_back_to_the_board_zero_level() {
    let mut scanner = reference_scanner(0, 2482);
    let travel = Travel::new();
    calibrate(&mut scanner, &travel);
    assert_eq!(
        scanner
            .sensor(Position::new(2, 7))
            .unwrap()
            .single()
            .rest_level,
        500
    );
    assert_eq!(
        scanner
            .sensor(Position::new(3, 15))
            .unwrap()
            .single()
            .rest_level,
        0
    );
}

#[test]
fn a_scan_of_four_inputs_and_sixteen_addresses_converts_every_key_once_in_either_mode() {
    let mut scanner = reference_scanner(2400, 2400);
    let travel = Travel::new();
    calibrate(&mut scanner, &travel);
    let mut now_ms = 0;
    for _ in 0..=SETTLING_CYCLES {
        cycle(&mut scanner, &travel, now_ms);
        now_ms += 1;
    }
    assert_eq!(scanner.mode(), Some(Mode::Single));
    assert_eq!(cycle(&mut scanner, &travel, now_ms), 1 + 4 * 16);
    assert!(scanner.events().is_empty());
    scanner.request_mode(Mode::Parallel);
    for _ in 0..=SETTLING_CYCLES {
        now_ms += 1;
        cycle(&mut scanner, &travel, now_ms);
    }
    assert_eq!(scanner.mode(), Some(Mode::Parallel));
    assert_eq!(cycle(&mut scanner, &travel, now_ms + 1), 1 + 16);
    assert!(scanner.events().is_empty());
}

#[test]
fn a_key_at_the_last_address_reports_its_travel_against_the_measured_reference() {
    let mut scanner = reference_scanner(2400, 2400);
    let travel = Travel::new();
    calibrate(&mut scanner, &travel);
    let mut now_ms = 0;
    for _ in 0..=SETTLING_CYCLES {
        cycle(&mut scanner, &travel, now_ms);
        now_ms += 1;
    }
    let position = Position::new(3, 15);
    scanner.converter_mut().readings[3][15] = 2400 + 700;
    cycle(&mut scanner, &travel, now_ms);
    let reported: Vec<_> = scanner
        .events()
        .iter()
        .map(|event| (event.position, event.key, event.travel))
        .collect();
    assert_eq!(reported, [(position, Key { row: 4, column: 15 }, 3386)]);
    assert_eq!(travel.get(position), 3386);
}

#[test]
fn idle_recalibration_by_standard_deviation_measures_two_inputs_per_batch() {
    let mut scanner = reference_scanner(2400, 2400);
    let travel = Travel::new();
    calibrate(&mut scanner, &travel);
    for now_ms in 0..1011 {
        cycle(&mut scanner, &travel, now_ms);
        scanner.converter_mut().operations.clear();
    }
    for input in 0..2 {
        let sensor = scanner.sensor(Position::new(input, 15)).unwrap();
        assert_eq!(sensor.noise().state(), NoiseState::Done);
        assert_eq!(sensor.rest_level(), 2);
        assert_eq!(sensor.single().rest_level, 2);
    }
    assert!(
        scanner
            .sensor(Position::new(2, 15))
            .unwrap()
            .is_recalibrating()
    );
    let recalibration = scanner.recalibration();
    assert_eq!(recalibration.batching(), Batching::InputPairs);
    assert_eq!(recalibration.batch(), 1);
    assert!(recalibration.is_running());
}
