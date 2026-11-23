use openwoot::{
    keys::Key,
    scan::{Mode, Travel},
};

use super::{
    simulated::Operation,
    support::{KEYS, PRESSED, SETTLING_CYCLES, cycle, event_travels, scanner, settled},
};

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
        [(Key { row: 1, column: 12 }, 3718)]
    );
}

#[test]
fn a_switch_back_to_the_single_input_scan_settles_and_converts_every_key_again() {
    let mut scanner = scanner();
    let travel = Travel::new();
    let mut now_ms = settled(&mut scanner, &travel);
    scanner.request_mode(Mode::Parallel);
    for _ in 0..=SETTLING_CYCLES {
        cycle(&mut scanner, &travel, now_ms);
        now_ms += 1;
    }
    assert_eq!(scanner.mode(), Some(Mode::Parallel));
    scanner.converter_mut().operations.clear();
    scanner.request_mode(Mode::Single);
    cycle(&mut scanner, &travel, now_ms);
    assert_eq!(scanner.mode(), Some(Mode::Single));
    assert!(
        scanner
            .converter()
            .operations
            .contains(&Operation::EnterSingle(0))
    );
    assert!(scanner.is_settling());
    for _ in 0..SETTLING_CYCLES {
        now_ms += 1;
        cycle(&mut scanner, &travel, now_ms);
    }
    assert!(!scanner.is_settling());
    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 700);
    now_ms += 1;
    assert_eq!(cycle(&mut scanner, &travel, now_ms), 1 + KEYS);
    assert_eq!(event_travels(&scanner), [(Key { row: 3, column: 4 }, 3386)]);
}
