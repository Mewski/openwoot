use openwoot::scan::{Mode, Position, Travel};

use super::{
    simulated::Operation,
    support::{
        PRESSED, SETTLING_CYCLES, calibrate, cycle, rest_everywhere, scanner, settled, wobble,
    },
};

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
fn a_sensor_without_any_spread_gets_the_smallest_noise_band() {
    let mut scanner = scanner();
    scanner.converter_mut().press(0, 0, 300);
    let travel = Travel::new();
    calibrate(&mut scanner, &travel);
    let quiet = scanner.sensor(Position::new(0, 0)).unwrap();
    assert_eq!(
        (quiet.single().rest_level, quiet.single().noise_band),
        (300, 1)
    );
    assert_eq!(
        (quiet.parallel().rest_level, quiet.parallel().noise_band),
        (305, 1)
    );
    let absent = scanner.sensor(Position::new(5, 13)).unwrap();
    assert_eq!(
        (absent.single().rest_level, absent.single().noise_band),
        (0, 0)
    );
    assert_eq!(
        (absent.parallel().rest_level, absent.parallel().noise_band),
        (5, 0)
    );
}

#[test]
fn a_second_sweep_loads_its_calibration_before_the_keys_are_evaluated() {
    let mut scanner = scanner();
    scanner.converter_mut().noise = Some(wobble);
    rest_everywhere(&mut scanner, 300);
    let travel = Travel::new();
    let start_ms = settled(&mut scanner, &travel);
    calibrate(&mut scanner, &travel);
    for now_ms in (start_ms..).take(3 * SETTLING_CYCLES as usize) {
        cycle(&mut scanner, &travel, now_ms);
        assert!(scanner.events().is_empty());
    }
    assert_eq!(scanner.mode(), Some(Mode::Single));
    assert!(!scanner.is_settling());
    assert_eq!(scanner.sensor(PRESSED).unwrap().rest_level(), 302);
    assert_eq!(travel.get(PRESSED), 0);
}

#[test]
fn a_second_sweep_measures_every_multiplexer_in_its_parallel_phase() {
    let mut scanner = scanner();
    scanner.converter_mut().noise = Some(wobble);
    rest_everywhere(&mut scanner, 300);
    let travel = Travel::new();
    let start_ms = settled(&mut scanner, &travel);
    let position = Position::new(4, 13);
    let first = scanner.sensor(position).unwrap().parallel();
    assert_eq!((first.rest_level, first.noise_band), (307, 1));
    scanner.request_mode(Mode::Parallel);
    for now_ms in (start_ms..).take(SETTLING_CYCLES as usize + 1) {
        cycle(&mut scanner, &travel, now_ms);
    }
    assert_eq!(scanner.mode(), Some(Mode::Parallel));
    calibrate(&mut scanner, &travel);
    assert_eq!(scanner.sensor(position).unwrap().parallel(), first);
}

#[test]
fn a_sweep_restarted_in_its_parallel_phase_returns_to_one_input_per_conversion() {
    let mut scanner = scanner();
    scanner.converter_mut().noise = Some(wobble);
    rest_everywhere(&mut scanner, 300);
    let travel = Travel::new();
    scanner.start_sweep();
    while !scanner
        .converter()
        .operations
        .iter()
        .rev()
        .take(2)
        .any(|operation| matches!(operation, Operation::SweepEnterParallel(_)))
    {
        scanner.on_conversion(&travel);
    }
    scanner.converter_mut().operations.clear();
    calibrate(&mut scanner, &travel);
    assert_eq!(
        scanner.converter().operations.first(),
        Some(&Operation::EnterSingle(0))
    );
    assert!(!scanner.is_calibrating());
    let swept = scanner.sensor(Position::new(4, 13)).unwrap();
    assert_eq!(
        (swept.single().rest_level, swept.single().noise_band),
        (302, 1)
    );
    assert_eq!(
        (swept.parallel().rest_level, swept.parallel().noise_band),
        (307, 1)
    );
}

#[test]
fn a_key_held_during_the_sweep_keeps_its_bottom_level_above_its_rest_level() {
    let mut scanner = scanner();
    scanner.converter_mut().noise = Some(wobble);
    rest_everywhere(&mut scanner, 300);
    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 1100);
    let travel = Travel::new();
    let mut now_ms = settled(&mut scanner, &travel);
    for _ in 0..3 {
        let released = [300; 30];
        let pressed = [1150; 41];
        for magnitude in released.into_iter().chain([600, 1000]).chain(pressed) {
            scanner
                .converter_mut()
                .press(PRESSED.input(), PRESSED.address(), magnitude);
            let sample_us = scanner.converter().now_us.wrapping_add(600);
            scanner.converter_mut().now_us = sample_us;
            cycle(&mut scanner, &travel, now_ms);
            now_ms += 1;
        }
        let sensor = scanner.sensor(PRESSED).unwrap();
        assert!(
            sensor.bottom_level() >= sensor.rest_level() + 50,
            "bottom level {} at rest level {}",
            sensor.bottom_level(),
            sensor.rest_level()
        );
        assert_eq!(scanner.events().len(), 1);
        assert!(scanner.events().iter().all(|event| event.travel > 0));
    }
}
