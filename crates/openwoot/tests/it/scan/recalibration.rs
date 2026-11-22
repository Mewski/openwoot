use openwoot::{
    analog::{NoiseState, NoiseStatistics},
    scan::{Batching, Position, Travel},
};

use super::{calibrate, cycle, scanner, settled, wobble};

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
