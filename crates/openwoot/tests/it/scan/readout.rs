use std::{
    pin::pin,
    task::{Context, Poll, Waker},
};

use openwoot::{
    keys::Key,
    protocol::Handler,
    scan::{Mode, Position, SensorReadout, Travel},
};
use openwoot_protocol::{
    FirmwareVersion, PROTOCOL_VERSION, Version, decode_response, encode_request,
    endpoints::GetSensorValues,
    report,
    sensors::{Reading, Values},
};

use super::support::{
    Bench80, KEYS, PRESSED, SETTLING_CYCLES, cycle, scanner, scanner_with_order, settled, wobble,
};
use crate::support::stock_identity;

const ROWS: usize = 6;
const COLUMNS: usize = 17;
const VERSION: Version = Version {
    protocol: PROTOCOL_VERSION,
    firmware: FirmwareVersion {
        major: 0,
        minor: 1,
        patch: 0,
    },
};

fn read(
    readout: &'static SensorReadout,
    scanner: &mut Bench80,
    travel: &Travel<6, 15>,
    reading: Reading,
) -> Option<Values> {
    let mut handler = Handler::new(VERSION, stock_identity(None)).with_sensors(readout);
    let mut request = [0; report::MAX_LEN];
    let request = encode_request::<GetSensorValues>(1, &reading, &mut request).ok()?;
    let mut response = [0; report::MAX_LEN];
    let mut context = Context::from_waker(Waker::noop());
    let mut answer = pin!(handler.respond(request, &mut response));
    assert!(answer.as_mut().poll(&mut context).is_pending());
    readout.serve::<ROWS, COLUMNS, _, _>(scanner, travel);
    let Poll::Ready(answer) = answer.as_mut().poll(&mut context) else {
        return None;
    };
    decode_response::<GetSensorValues>(1, answer.reply?)
        .ok()??
        .ok()
}

fn key_at(scanner: &Bench80, position: Position) -> Option<Key> {
    scanner.map().key(position)
}

fn value(values: &Values, key: Key) -> Option<u16> {
    values.get(key.index(COLUMNS)).copied().flatten()
}

fn rest_levels(scanner: &Bench80, position: Position) -> Option<(u16, u16, u16)> {
    let sensor = scanner.sensor(position)?;
    Some((
        sensor.rest_level(),
        sensor.single().rest_level,
        sensor.parallel().rest_level,
    ))
}

fn readout_rest_levels(
    readout: &'static SensorReadout,
    scanner: &mut Bench80,
    travel: &Travel<6, 15>,
    key: Key,
) -> (Option<u16>, Option<u16>, Option<u16>) {
    let mut level =
        |reading| read(readout, scanner, travel, reading).and_then(|values| value(&values, key));
    (
        level(Reading::RestLevel),
        level(Reading::SingleRestLevel),
        level(Reading::ParallelRestLevel),
    )
}

#[test]
fn the_readout_reports_the_rest_level_of_each_scan_mode_beside_the_one_in_effect() {
    static READOUT: SensorReadout = SensorReadout::new();
    let readout = &READOUT;
    let mut scanner = scanner();
    scanner.converter_mut().noise = Some(wobble);
    let position = Position::new(0, 0);
    scanner
        .converter_mut()
        .press(position.input(), position.address(), 300);
    let travel = Travel::new();
    let now_ms = settled(&mut scanner, &travel);
    let key = key_at(&scanner, position).unwrap();

    let (in_effect, single, parallel) = rest_levels(&scanner, position).unwrap();
    assert_eq!(in_effect, single);
    assert_ne!(single, parallel);
    assert_eq!(
        readout_rest_levels(readout, &mut scanner, &travel, key),
        (Some(single), Some(single), Some(parallel))
    );

    scanner.request_mode(Mode::Parallel);
    for offset in 0..=SETTLING_CYCLES {
        cycle(&mut scanner, &travel, now_ms + offset);
    }
    assert_eq!(scanner.mode(), Some(Mode::Parallel));
    let (in_effect, single, parallel) = rest_levels(&scanner, position).unwrap();
    assert_eq!(in_effect, parallel);
    assert_eq!(
        readout_rest_levels(readout, &mut scanner, &travel, key),
        (Some(parallel), Some(single), Some(parallel))
    );
}

#[test]
fn the_readout_flags_the_keys_whose_sensor_value_is_above_its_rest_threshold() {
    static READOUT: SensorReadout = SensorReadout::new();
    let readout = &READOUT;
    let mut scanner = scanner();
    let travel = Travel::new();
    let now_ms = settled(&mut scanner, &travel);
    scanner
        .converter_mut()
        .press(PRESSED.input(), PRESSED.address(), 700);
    cycle(&mut scanner, &travel, now_ms);

    let above = read(readout, &mut scanner, &travel, Reading::AboveRest).unwrap();
    assert_eq!(value(&above, key_at(&scanner, PRESSED).unwrap()), Some(1));
    assert_eq!(
        value(&above, key_at(&scanner, Position::new(0, 0)).unwrap()),
        Some(0)
    );
    assert_eq!(above.iter().flatten().filter(|&&flag| flag == 1).count(), 1);
    assert_eq!(above.iter().flatten().count(), KEYS);
}

#[test]
fn the_single_input_scan_order_walks_the_keys_input_by_input() {
    static READOUT: SensorReadout = SensorReadout::new();
    let readout = &READOUT;
    let mut scanner = scanner();
    let travel = Travel::new();
    settled(&mut scanner, &travel);
    assert_eq!(scanner.mode(), Some(Mode::Single));

    let order = read(readout, &mut scanner, &travel, Reading::ScanOrder).unwrap();
    let at = |input, address| {
        value(
            &order,
            key_at(&scanner, Position::new(input, address)).unwrap(),
        )
    };
    assert_eq!(at(0, 0), Some(0));
    assert_eq!(at(0, 13), Some(13));
    assert_eq!(at(1, 0), Some(14));
    assert_eq!(at(1, 12), Some(26));
    assert_eq!(at(1, 14), Some(27));
    assert_eq!(at(2, 0), Some(28));
    assert_eq!(at(5, 13), Some(83));
    let mut ranks: Vec<u16> = order.iter().flatten().copied().collect();
    ranks.sort_unstable();
    assert_eq!(ranks, (0..u16::try_from(KEYS).unwrap()).collect::<Vec<_>>());
}

#[test]
fn the_six_input_scan_order_walks_the_addresses_with_the_inputs_in_conversion_rank() {
    static READOUT: SensorReadout = SensorReadout::new();
    let readout = &READOUT;
    let mut scanner = scanner_with_order([5, 4, 3, 2, 1, 0]);
    let travel = Travel::new();
    let now_ms = settled(&mut scanner, &travel);
    scanner.request_mode(Mode::Parallel);
    cycle(&mut scanner, &travel, now_ms);
    assert_eq!(scanner.mode(), Some(Mode::Parallel));

    let order = read(readout, &mut scanner, &travel, Reading::ScanOrder).unwrap();
    let at = |input, address| {
        value(
            &order,
            key_at(&scanner, Position::new(input, address)).unwrap(),
        )
    };
    assert_eq!(at(5, 0), Some(0));
    assert_eq!(at(4, 0), Some(1));
    assert_eq!(at(0, 0), Some(5));
    assert_eq!(at(5, 1), Some(6));
    assert_eq!(at(1, 12), Some(76));
    assert_eq!(at(5, 13), Some(78));
    assert_eq!(at(0, 13), Some(82));
    assert_eq!(at(1, 14), Some(83));
    let mut ranks: Vec<u16> = order.iter().flatten().copied().collect();
    ranks.sort_unstable();
    assert_eq!(ranks, (0..u16::try_from(KEYS).unwrap()).collect::<Vec<_>>());
}
