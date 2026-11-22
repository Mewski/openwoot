use openwoot::scan::{Map, MatrixKey, Position, Scanner, Travel};

use super::{ReferenceBench, Simulated, calibrate};

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
