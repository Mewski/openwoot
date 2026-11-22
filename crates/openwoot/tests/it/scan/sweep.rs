use openwoot::scan::{Mode, Position, Travel};

use super::{Operation, SETTLING_CYCLES, calibrate, cycle, scanner, wobble};

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
