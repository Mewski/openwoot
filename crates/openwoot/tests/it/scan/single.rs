use openwoot::{
    analog::ReboundState,
    keys::Key,
    scan::{Map, Mode, Scanner, Travel},
};

use super::{
    simulated::{Operation, Simulated},
    support::{Bench80, KEYS, PRESSED, cycle, event_travels, scanner, sensors, settled},
};

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
    let key = Key { row: 3, column: 4 };

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
        assert_eq!(event_travels(&scanner), [(Key { row: 3, column: 4 }, 0)]);
    }
}

#[test]
fn holding_the_events_of_a_key_holds_those_of_its_sensor() {
    let mut scanner = scanner();
    let travel = Travel::new();
    let now_ms = settled(&mut scanner, &travel);
    let key = Key { row: 3, column: 4 };
    scanner.hold_events(key, true);
    cycle(&mut scanner, &travel, now_ms);
    assert_eq!(event_travels(&scanner), [(key, 0)]);
    scanner.hold_events(key, false);
    cycle(&mut scanner, &travel, now_ms + 1);
    assert_eq!(event_travels(&scanner), []);
    scanner.hold_events(Key { row: 7, column: 31 }, true);
    cycle(&mut scanner, &travel, now_ms + 2);
    assert_eq!(event_travels(&scanner), []);
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
fn a_single_input_cycle_of_85_conversions_samples_every_key_in_513_microseconds() {
    const CONVERSION_NS: u32 = 375;
    let mut scanner = scanner();
    let travel = Travel::new();
    let now_ms = settled(&mut scanner, &travel);
    let conversions = u32::try_from(cycle(&mut scanner, &travel, now_ms)).unwrap();
    let delays_us: u32 = scanner
        .converter()
        .operations
        .iter()
        .map(|operation| match *operation {
            Operation::TriggerAt(delay) | Operation::TriggerAfter(delay) => u32::from(delay),
            _ => 0,
        })
        .sum();
    assert_eq!((conversions, delays_us), (85, 481));
    assert_eq!(delays_us * 1000 + conversions * CONVERSION_NS, 512_875);
}

#[test]
fn a_release_during_the_settling_cycles_of_a_mode_switch_is_reported_when_they_end() {
    let mut scanner = scanner();
    let travel = Travel::new();
    let mut now_ms = settled(&mut scanner, &travel);
    let key = Key { row: 3, column: 4 };
    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 700);
    cycle(&mut scanner, &travel, now_ms);
    assert_eq!(event_travels(&scanner), [(key, 3386)]);
    scanner.request_mode(Mode::Parallel);
    now_ms += 1;
    cycle(&mut scanner, &travel, now_ms);
    assert!(scanner.is_settling());
    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 0);
    let mut reported = Vec::new();
    for _ in 0..10 {
        now_ms += 1;
        cycle(&mut scanner, &travel, now_ms);
        reported.extend(event_travels(&scanner));
    }
    assert!(!scanner.is_settling());
    assert_eq!(reported, [(key, 0)]);
    assert_eq!(travel.get(PRESSED), 0);
}

#[test]
fn a_key_that_is_down_when_raw_scanning_starts_is_reported_at_rest_once() {
    let mut scanner = scanner();
    let travel = Travel::new();
    let mut now_ms = settled(&mut scanner, &travel);
    let key = Key { row: 3, column: 4 };
    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 700);
    cycle(&mut scanner, &travel, now_ms);
    assert_eq!(event_travels(&scanner), [(key, 3386)]);
    scanner.set_raw_scanning(true);
    now_ms += 1;
    cycle(&mut scanner, &travel, now_ms);
    assert_eq!(event_travels(&scanner), [(key, 0)]);
    assert_eq!(travel.get(PRESSED), 0);
    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 0);
    now_ms += 1;
    cycle(&mut scanner, &travel, now_ms);
    assert!(scanner.events().is_empty());
    scanner.set_raw_scanning(false);
    for _ in 0..3 {
        now_ms += 1;
        cycle(&mut scanner, &travel, now_ms);
        assert!(scanner.events().is_empty());
    }
    assert_eq!(travel.get(PRESSED), 0);
}

#[test]
fn a_map_without_keys_on_the_first_multiplexer_ends_its_cycle_at_the_last_key() {
    let mut codes = [[0xFF; 16]; 6];
    for (row, keys) in (0..).zip(codes.iter_mut()).skip(1) {
        for (column, code) in (0..15).zip(keys.iter_mut()) {
            *code = Key { row, column }.code();
        }
    }
    let mut scanner: Bench80 = Scanner::new(
        Simulated::default(),
        Map::from_codes(codes),
        [0, 1, 2, 3, 4, 5],
        sensors(),
    );
    let travel = Travel::new();
    let now_ms = settled(&mut scanner, &travel);
    assert_eq!(cycle(&mut scanner, &travel, now_ms), 1 + 5 * 15);
    scanner.converter_mut().press(1, 0, 700);
    assert_eq!(cycle(&mut scanner, &travel, now_ms + 1), 1 + 5 * 15);
    assert_eq!(event_travels(&scanner), [(Key { row: 1, column: 0 }, 3386)]);
}
