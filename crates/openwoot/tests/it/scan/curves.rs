use openwoot::{
    analog::{Switch, SwitchModel},
    keys::Key,
    scan::{Mode, Travel},
};

use super::support::{
    Bench80, SETTLING_CYCLES, curve_tables, cycle, event_travels, scanner,
    scanner_without_curve_tables, settled,
};

const PRESSES: [(u8, u8); 6] = [(1, 5), (0, 0), (2, 3), (3, 7), (4, 1), (5, 9)];
const MAGNITUDES: [u16; 7] = [0, 120, 400, 700, 1100, 2400, 0];
const FIRST_MODELS: [(u8, Option<u8>); 3] = [(0, None), (2, Some(10)), (9, Some(30))];
const SECOND_MODELS: [(u8, Option<u8>); 3] = [(2, Some(10)), (13, None), (20, Some(5))];

type Reports = Vec<Vec<(Key, u16)>>;

fn switch_for(models: [(u8, Option<u8>); 3]) -> impl Fn(Key) -> Switch {
    move |key| {
        let (id, offset) = models
            .get(usize::from(key.code()) % models.len())
            .copied()
            .unwrap_or_default();
        Switch::new(SwitchModel::new(id).unwrap_or(SwitchModel::DEFAULT), offset)
    }
}

fn press_round(scanner: &mut Bench80, travel: &Travel<6, 15>, now_ms: &mut u32) -> Reports {
    let mut reports = Vec::new();
    for magnitude in MAGNITUDES {
        for (rank, (input, address)) in (0..).zip(PRESSES) {
            let pressed = if magnitude == 0 {
                0
            } else {
                magnitude + rank * 37
            };
            scanner.converter_mut().press(input, address, pressed);
        }
        cycle(scanner, travel, *now_ms);
        *now_ms += 1;
        reports.push(event_travels(scanner));
    }
    reports
}

fn reports_through_switch_changes(scanner: &mut Bench80) -> Reports {
    let travel = Travel::new();
    let mut now_ms = settled(scanner, &travel);
    let mut reports = Vec::new();
    scanner.apply_switches(switch_for(FIRST_MODELS));
    reports.extend(press_round(scanner, &travel, &mut now_ms));
    scanner.apply_switches(switch_for(SECOND_MODELS));
    reports.extend(press_round(scanner, &travel, &mut now_ms));
    scanner.request_mode(Mode::Parallel);
    for _ in 0..=SETTLING_CYCLES {
        cycle(scanner, &travel, now_ms);
        now_ms += 1;
    }
    reports.extend(press_round(scanner, &travel, &mut now_ms));
    scanner.apply_switches(switch_for(FIRST_MODELS));
    reports.extend(press_round(scanner, &travel, &mut now_ms));
    reports
}

#[test]
fn a_scanner_with_fewer_curve_tables_than_curves_in_use_reports_like_one_without_tables() {
    let compact = reports_through_switch_changes(&mut scanner_without_curve_tables());
    let one_table = reports_through_switch_changes(
        &mut scanner_without_curve_tables().with_curve_tables(curve_tables::<1>()),
    );
    let two_tables = reports_through_switch_changes(
        &mut scanner_without_curve_tables().with_curve_tables(curve_tables::<2>()),
    );
    assert_eq!(one_table, compact);
    assert_eq!(two_tables, compact);
    assert!(
        compact
            .iter()
            .flatten()
            .filter(|&&(_, travel)| travel != 0 && travel != 4095)
            .count()
            > 20
    );
}

#[test]
fn a_scanner_with_a_table_for_every_curve_in_use_reports_like_one_without_tables() {
    let compact = reports_through_switch_changes(&mut scanner_without_curve_tables());
    let cached = reports_through_switch_changes(&mut scanner());
    assert_eq!(cached, compact);
}
