use embassy_futures::block_on;
use openwoot::scan::{Mode, Scanner, Shared};

use super::{Bench, Bench80, PRESSED, SETTLING_CYCLES, scanner};

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
