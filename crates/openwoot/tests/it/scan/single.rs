use openwoot::{
    analog::ReboundState,
    scan::{MatrixKey, Travel},
};

use super::{Bench80, KEYS, Operation, PRESSED, cycle, event_travels, scanner, settled};

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
