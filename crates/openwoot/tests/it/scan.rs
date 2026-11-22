use embassy_futures::block_on;
use openwoot::{
    analog::{Batching, NoiseState, NoiseStatistics, ReboundState},
    scan::{Map, MatrixKey, Mode, Position, Scanner, Shared, Travel},
};

use crate::support::{Bench, Operation, ReferenceBench, Simulated, calibrate, cycle};

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
const KEYS: usize = 84;
const SETTLING_CYCLES: u32 = 4;
const PRESSED: Position = Position::new(1, 5);

type Bench80 = Scanner<Bench, 6, 15>;

fn wobble(conversion: u32) -> i16 {
    (conversion % 3) as i16
}

fn scanner() -> Bench80 {
    Scanner::new(Simulated::default(), MAP, ORDER)
}

fn settled(scanner: &mut Bench80, travel: &Travel<6, 15>) -> u32 {
    calibrate(scanner, travel);
    for now_ms in 0..=SETTLING_CYCLES {
        cycle(scanner, travel, now_ms);
    }
    scanner.converter_mut().clear();
    SETTLING_CYCLES + 1
}

fn event_travels(scanner: &Bench80) -> Vec<(MatrixKey, u16)> {
    scanner
        .events()
        .iter()
        .map(|event| (event.key, event.travel))
        .collect()
}

#[test]
fn the_sweep_measures_rest_level_and_noise_band_of_both_scan_modes() {
    let mut scanner = scanner();
    scanner.converter_mut().noise = Some(wobble);
    scanner.converter_mut().press(0, 0, 300);
    let travel = Travel::new();
    assert_eq!(
        calibrate(&mut scanner, &travel),
        1 + 6 * 15 * 100 + 15 * 100
    );
    assert!(!scanner.is_calibrating());

    let pressed = scanner.sensor(Position::new(0, 0)).unwrap();
    assert_eq!(
        (pressed.single().rest_level, pressed.single().noise_band),
        (302, 1)
    );
    assert_eq!(
        (pressed.parallel().rest_level, pressed.parallel().noise_band),
        (307, 1)
    );
    assert_eq!(pressed.bottom_level(), 1200);
    let resting = scanner.sensor(Position::new(5, 13)).unwrap();
    assert_eq!(
        (resting.single().rest_level, resting.single().noise_band),
        (0, 0)
    );
    assert_eq!(
        (resting.parallel().rest_level, resting.parallel().noise_band),
        (5, 0)
    );
}

#[test]
fn the_sweep_walks_every_input_and_then_converts_all_inputs_together() {
    let mut scanner = scanner();
    let travel = Travel::new();
    calibrate(&mut scanner, &travel);
    let operations = &scanner.converter().operations;
    assert_eq!(
        operations[..3],
        [
            Operation::SelectInput(0, 0),
            Operation::SelectAddress(0),
            Operation::TriggerAfter(254),
        ]
    );
    let switches: Vec<_> = operations
        .iter()
        .filter(|operation| matches!(operation, Operation::SelectInput(..)))
        .collect();
    assert_eq!(switches.len(), 1 + 6);
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation, Operation::SweepEnterParallel(true)))
            .count(),
        1
    );
    assert_eq!(operations.last(), Some(&Operation::SelectAddress(0)));
}

#[test]
fn the_first_cycle_after_the_sweep_loads_the_single_input_calibration_and_settles() {
    let mut scanner = scanner();
    let travel = Travel::new();
    calibrate(&mut scanner, &travel);
    scanner.converter_mut().clear();
    cycle(&mut scanner, &travel, 0);
    assert_eq!(scanner.mode(), Some(Mode::Single));
    assert!(scanner.is_settling());
    assert_eq!(
        scanner.converter().operations[..2],
        [Operation::EnterSingle(0), Operation::TriggerAfter(54),]
    );
    for now_ms in 1..SETTLING_CYCLES {
        cycle(&mut scanner, &travel, now_ms);
        assert!(scanner.is_settling());
    }
    cycle(&mut scanner, &travel, SETTLING_CYCLES);
    assert!(!scanner.is_settling());
}

#[test]
fn a_single_input_cycle_converts_every_key_once_and_switches_inputs_five_times() {
    let mut scanner = scanner();
    let travel = Travel::new();
    let now_ms = settled(&mut scanner, &travel);
    assert_eq!(cycle(&mut scanner, &travel, now_ms), 1 + KEYS);
    let operations = &scanner.converter().operations;
    assert_eq!(operations.first(), Some(&Operation::TriggerAt(55)));
    assert_eq!(
        operations
            .iter()
            .filter(|operation| **operation == Operation::TriggerAfter(54))
            .count(),
        5
    );
    assert_eq!(operations.last(), Some(&Operation::EndCycle(900)));
    assert!(scanner.events().is_empty());
}

#[test]
fn a_pressed_key_reports_its_travel_once_per_cycle_and_its_release_once() {
    let mut scanner = scanner();
    let travel = Travel::new();
    let mut now_ms = settled(&mut scanner, &travel);
    let key = MatrixKey { row: 3, column: 4 };

    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 700);
    cycle(&mut scanner, &travel, now_ms);
    assert_eq!(event_travels(&scanner), [(key, 3386)]);
    assert_eq!(travel.get(PRESSED), 3386);
    now_ms += 1;
    cycle(&mut scanner, &travel, now_ms);
    assert_eq!(event_travels(&scanner), [(key, 3386)]);

    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 0);
    now_ms += 1;
    cycle(&mut scanner, &travel, now_ms);
    assert_eq!(event_travels(&scanner), [(key, 0)]);
    assert_eq!(travel.get(PRESSED), 0);
    now_ms += 1;
    cycle(&mut scanner, &travel, now_ms);
    assert!(scanner.events().is_empty());
}

#[test]
fn a_changed_reading_from_the_fourth_input_on_is_converted_twice() {
    let mut scanner = scanner();
    let travel = Travel::new();
    let now_ms = settled(&mut scanner, &travel);
    scanner.converter_mut().press(3, 2, 500);
    assert_eq!(cycle(&mut scanner, &travel, now_ms), 2 + KEYS);
    scanner.converter_mut().press(1, 2, 500);
    assert_eq!(cycle(&mut scanner, &travel, now_ms + 1), 1 + KEYS);
}

#[test]
fn the_parallel_scan_converts_every_address_once_and_rechecks_changed_inputs() {
    let mut scanner = scanner();
    let travel = Travel::new();
    let mut now_ms = settled(&mut scanner, &travel);
    scanner.request_mode(Mode::Parallel);
    cycle(&mut scanner, &travel, now_ms);
    assert_eq!(scanner.mode(), Some(Mode::Parallel));
    assert_eq!(
        scanner.converter().operations[0],
        Operation::EnterParallel(0)
    );
    for _ in 0..=SETTLING_CYCLES {
        now_ms += 1;
        cycle(&mut scanner, &travel, now_ms);
    }
    now_ms += 1;
    assert_eq!(cycle(&mut scanner, &travel, now_ms), 1 + 15);

    scanner.converter_mut().press(4, 13, 900);
    now_ms += 1;
    assert_eq!(cycle(&mut scanner, &travel, now_ms), 1 + 15 + 1);
    assert_eq!(
        event_travels(&scanner),
        [(MatrixKey { row: 1, column: 12 }, 3718)]
    );
}

#[test]
fn raw_scanning_shows_the_unfiltered_magnitude_and_queues_no_events() {
    let mut scanner = scanner();
    let travel = Travel::new();
    let now_ms = settled(&mut scanner, &travel);
    scanner.set_raw_scanning(true);
    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 700);
    cycle(&mut scanner, &travel, now_ms);
    assert!(scanner.events().is_empty());
    assert_eq!(scanner.sensor(PRESSED).unwrap().value(), 700);
}

#[test]
fn a_key_that_holds_its_events_reports_at_rest_every_cycle() {
    let mut scanner = scanner();
    let travel = Travel::new();
    let now_ms = settled(&mut scanner, &travel);
    scanner.sensor_mut(PRESSED).unwrap().hold_events(true);
    for offset in 0..3 {
        cycle(&mut scanner, &travel, now_ms + offset);
        assert_eq!(
            event_travels(&scanner),
            [(MatrixKey { row: 3, column: 4 }, 0)]
        );
    }
}

#[test]
fn the_rebound_filter_reports_a_bounce_after_a_release_as_no_travel() {
    let mut scanner = scanner();
    let travel = Travel::new();
    let mut now_ms = settled(&mut scanner, &travel);
    scanner.set_rebound_filter(true);
    let mut step = |scanner: &mut Bench80, magnitude: u16| {
        scanner
            .converter_mut()
            .press(PRESSED.input(), PRESSED.address(), magnitude);
        cycle(scanner, &travel, now_ms);
        now_ms += 1;
        scanner
            .events()
            .iter()
            .map(|event| event.travel)
            .collect::<Vec<_>>()
    };
    assert_eq!(step(&mut scanner, 1000), [3858]);
    step(&mut scanner, 800);
    assert_eq!(step(&mut scanner, 0), [0]);
    assert_eq!(
        scanner.sensor(PRESSED).unwrap().rebound().state(),
        ReboundState::Released
    );
    assert_eq!(step(&mut scanner, 150), [0]);
    assert_eq!(
        scanner.sensor(PRESSED).unwrap().rebound().state(),
        ReboundState::Rebound
    );
}

#[test]
fn idle_recalibration_measures_two_inputs_per_batch() {
    let mut scanner = scanner();
    let travel = Travel::new();
    calibrate(&mut scanner, &travel);
    for now_ms in 0..1011 {
        cycle(&mut scanner, &travel, now_ms);
    }
    for input in 0..2 {
        let sensor = scanner.sensor(Position::new(input, 0)).unwrap();
        assert_eq!(sensor.noise().state(), NoiseState::Done);
        assert_eq!(sensor.rest_level(), 5);
        assert_eq!(sensor.single().rest_level, 5);
    }
    let sampling = scanner.sensor(Position::new(2, 0)).unwrap();
    assert!(sampling.is_recalibrating());
    assert_eq!(sampling.rest_level(), 0);
    assert!(
        !scanner
            .sensor(Position::new(5, 0))
            .unwrap()
            .is_recalibrating()
    );
    let recalibration = scanner.recalibration();
    assert_eq!(recalibration.batching(), Batching::InputPairs);
    assert_eq!(recalibration.batch(), 1);
    assert!(recalibration.is_running());
}

#[test]
fn key_activity_stops_recalibration_and_holds_it() {
    let mut scanner = scanner();
    scanner.converter_mut().noise = Some(wobble);
    scanner.converter_mut().press(0, 0, 300);
    let travel = Travel::new();
    let now_ms = settled(&mut scanner, &travel);
    assert!(scanner.recalibration().is_running());
    scanner.converter_mut().press(0, 0, 400);
    cycle(&mut scanner, &travel, now_ms);
    assert!(!scanner.recalibration().is_running());
    assert!(scanner.recalibration().is_held());
    assert!(
        !scanner
            .sensor(Position::new(0, 1))
            .unwrap()
            .is_recalibrating()
    );
}

#[test]
fn the_hand_off_lends_the_scanner_to_the_interrupt_for_one_cycle() {
    let shared: Shared<Bench, 6, 15> = Shared::new();
    assert!(shared.install(scanner()));
    assert!(!shared.start());
    assert_eq!(shared.with(Scanner::start_sweep), Some(()));
    assert!(shared.start());
    assert!(shared.is_running());
    assert!(shared.with(|_| ()).is_none());
    assert!(!shared.start());
    while shared.is_running() {
        shared.on_conversion();
    }
    block_on(shared.finished());
    assert_eq!(shared.with(|scanner| scanner.is_calibrating()), Some(false));

    assert_eq!(
        shared.with(|scanner| {
            scanner
                .converter_mut()
                .press(PRESSED.input(), PRESSED.address(), 700);
            scanner.prepare(0);
        }),
        Some(())
    );
    assert!(shared.start());
    while shared.is_running() {
        shared.on_conversion();
    }
    assert_eq!(
        shared.with(|scanner| scanner.mode()),
        Some(Some(Mode::Single))
    );
}

#[test]
fn the_travel_table_is_shared_with_readers_outside_the_scan() {
    let shared: Shared<Bench, 6, 15> = Shared::new();
    assert!(shared.install(scanner()));
    let lend = |change: fn(&mut Bench80)| {
        assert!(shared.with(change).is_some());
        assert!(shared.start());
        while shared.is_running() {
            shared.on_conversion();
        }
    };
    lend(Bench80::start_sweep);
    for _ in 0..=SETTLING_CYCLES {
        lend(|scanner| scanner.prepare(0));
    }
    lend(|scanner| {
        scanner
            .converter_mut()
            .press(PRESSED.input(), PRESSED.address(), 700);
        scanner.prepare(0);
    });
    assert_eq!(shared.travel().get(PRESSED), 3386);
}

fn reference_scanner(reference: u16, reading: u16) -> Scanner<ReferenceBench, 4, 16> {
    let mut codes = [[0xFF; 16]; 4];
    for (row, keys) in (1..).zip(codes.iter_mut()) {
        for (column, code) in (0..).zip(keys.iter_mut()) {
            *code = MatrixKey { row, column }.code();
        }
    }
    let mut scanner: Scanner<ReferenceBench, 4, 16> =
        Scanner::new(Simulated::default(), Map::from_codes(codes), [0, 1, 2, 3]);
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
