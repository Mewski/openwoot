use openwoot::{
    analog::{NoiseState, NoiseStatistics},
    scan::{Batching, Position, Travel},
};

use super::support::{PRESSED, calibrate, cycle, rest_everywhere, scanner, settled, wobble};

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
fn a_rest_level_raised_above_a_lightly_loaded_key_reports_the_key_at_rest() {
    let mut scanner = scanner();
    scanner.converter_mut().noise = Some(wobble);
    rest_everywhere(&mut scanner, 300);
    let travel = Travel::new();
    let mut now_ms = settled(&mut scanner, &travel);
    for _ in 0..4000 {
        cycle(&mut scanner, &travel, now_ms);
        now_ms += 1;
        scanner.converter_mut().operations.clear();
    }
    let position = Position::new(2, 3);
    let key = scanner.map().key(position).unwrap();
    let rest_level = scanner.sensor(position).unwrap().rest_level();
    scanner.converter_mut().press(2, 3, 322);
    let mut reported = Vec::new();
    for _ in 0..30_000 {
        cycle(&mut scanner, &travel, now_ms);
        now_ms += 1;
        scanner.converter_mut().operations.clear();
        reported.extend(
            scanner
                .events()
                .iter()
                .filter(|event| event.key == key)
                .map(|event| event.travel),
        );
    }
    assert!(scanner.sensor(position).unwrap().rest_level() > rest_level);
    assert!(reported.first().is_some_and(|&first| first > 0));
    assert_eq!(reported.last(), Some(&0));
    assert_eq!(reported.iter().filter(|&&level| level == 0).count(), 1);
    assert_eq!(travel.get(position), 0);
}

#[test]
fn a_noise_free_key_that_is_held_still_holds_recalibration() {
    let mut scanner = scanner();
    rest_everywhere(&mut scanner, 300);
    let travel = Travel::new();
    let start_ms = settled(&mut scanner, &travel);
    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 900);
    for now_ms in (start_ms..).take(2500) {
        cycle(&mut scanner, &travel, now_ms);
        scanner.converter_mut().operations.clear();
    }
    assert!(scanner.recalibration().is_held());
    assert_eq!(scanner.sensor(PRESSED).unwrap().rest_level(), 300);
    assert_eq!(scanner.events().len(), 1);
    assert!(scanner.events().iter().all(|event| event.travel > 3000));
}

#[test]
fn a_key_held_still_stops_holding_recalibration_but_keeps_its_rest_level() {
    let mut scanner = scanner();
    scanner.converter_mut().noise = Some(wobble);
    rest_everywhere(&mut scanner, 300);
    let travel = Travel::new();
    let mut now_ms = settled(&mut scanner, &travel);
    let rest_level = scanner.sensor(PRESSED).unwrap().rest_level();
    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 340);
    for _ in 0..9000 {
        cycle(&mut scanner, &travel, now_ms);
        now_ms += 1;
        scanner.converter_mut().operations.clear();
    }
    assert!(scanner.recalibration().is_held());
    assert!(!scanner.recalibration().is_running());
    for _ in 0..7500 {
        cycle(&mut scanner, &travel, now_ms);
        now_ms += 1;
        scanner.converter_mut().operations.clear();
        assert!(
            scanner
                .events()
                .iter()
                .any(|event| event.position == PRESSED && event.travel > 0)
        );
    }
    let recalibration = scanner.recalibration();
    assert!(recalibration.is_running());
    assert_eq!(recalibration.batch(), 1);
    assert_eq!(
        scanner.sensor(Position::new(0, 0)).unwrap().noise().state(),
        NoiseState::Done
    );
    let held = scanner.sensor(PRESSED).unwrap();
    assert!(!held.is_recalibrating());
    assert_eq!(held.rest_level(), rest_level);
    assert!(travel.get(PRESSED) > 0);
}

#[test]
fn a_sensor_stuck_at_full_scale_reports_nothing_and_never_holds_recalibration() {
    let mut scanner = scanner();
    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 2047);
    let travel = Travel::new();
    let start_ms = settled(&mut scanner, &travel);
    for now_ms in (start_ms..).take(1100) {
        cycle(&mut scanner, &travel, now_ms);
        scanner.converter_mut().operations.clear();
        assert!(scanner.events().is_empty());
        assert!(!scanner.recalibration().is_held());
    }
    assert!(scanner.sensor(PRESSED).unwrap().rest_level() >= 2047);
    assert_eq!(travel.get(PRESSED), 0);
}
