use std::{
    pin::pin,
    task::{Context, Poll, Waker},
};

use embassy_futures::block_on;
use openwoot::{keys::Key, protocol::Handler, runtime::KeyTask, scan::SensorReadout, usb::Mode};
use openwoot_protocol::{
    Endpoint, FirmwareVersion, PROTOCOL_VERSION, Version,
    configuration::Rejection,
    decode_response, encode_request,
    endpoints::{GetSensorValues, SetRawScanning},
    report,
    sensors::{Reading, Values},
};
use serde::{Serialize, de::DeserializeOwned};

use super::{
    pipeline::{Bench80, FULL, World, storage},
    support::{COLUMNS, KEY_A, KEY_S, ROWS},
};
use crate::support::stock_identity;

const VERSION: Version = Version {
    protocol: PROTOCOL_VERSION,
    firmware: FirmwareVersion {
        major: 0,
        minor: 1,
        patch: 0,
    },
};
const KEY_WITHOUT_SENSOR: Key = Key::new(0, 16);

fn read(handler: &mut Handler, bench: &mut Bench80<'_>, reading: Reading) -> Option<Values> {
    let mut request = [0; report::MAX_LEN];
    let request = encode_request::<GetSensorValues>(1, &reading, &mut request).ok()?;
    let mut response = [0; report::MAX_LEN];
    let mut context = Context::from_waker(Waker::noop());
    let mut answer = pin!(handler.respond(request, &mut response));
    assert!(answer.as_mut().poll(&mut context).is_pending());
    bench.cycle();
    let Poll::Ready(answer) = answer.as_mut().poll(&mut context) else {
        return None;
    };
    decode_response::<GetSensorValues>(1, answer.reply?)
        .ok()??
        .ok()
}

fn send<E>(handler: &mut Handler, request: &E::Request) -> Option<E::Response>
where
    E: Endpoint,
    E::Request: Serialize,
    E::Response: DeserializeOwned,
{
    let mut encoded = [0; report::MAX_LEN];
    let encoded = encode_request::<E>(2, request, &mut encoded).ok()?;
    let mut response = [0; report::MAX_LEN];
    let answer = block_on(handler.respond(encoded, &mut response));
    decode_response::<E>(2, answer.reply?).ok()?
}

fn value(values: &Values, key: Key) -> Option<u16> {
    values.get(key.index(COLUMNS)).copied().flatten()
}

#[test]
fn the_key_task_answers_a_sensor_readout_after_its_next_cycle_in_key_order() {
    static SENSORS: SensorReadout = SensorReadout::new();
    let world = World::new();
    let mut settings = storage();
    let _model = world.model(&mut settings);
    let mut bench = Bench80::new(KeyTask {
        sensors: &SENSORS,
        ..world.task(Mode::NoGamepad)
    });
    let mut handler = Handler::new(VERSION, stock_identity(None)).with_sensors(&SENSORS);
    bench.press(KEY_A, FULL);
    let travel = read(&mut handler, &mut bench, Reading::Travel).unwrap();
    assert_eq!(travel.len(), ROWS * COLUMNS);
    assert!(value(&travel, KEY_A).unwrap() > 3000, "{travel:?}");
    assert_eq!(value(&travel, KEY_S), Some(0));
    assert_eq!(value(&travel, KEY_WITHOUT_SENSOR), None);
    let rest = read(&mut handler, &mut bench, Reading::RestLevel).unwrap();
    let bottom = read(&mut handler, &mut bench, Reading::BottomLevel).unwrap();
    assert!(value(&bottom, KEY_S).unwrap() > value(&rest, KEY_S).unwrap());
}

#[test]
fn raw_scanning_reports_the_sensor_values_and_holds_the_key_travel() {
    static SENSORS: SensorReadout = SensorReadout::new();
    let world = World::new();
    let mut settings = storage();
    let _model = world.model(&mut settings);
    let mut bench = Bench80::new(KeyTask {
        sensors: &SENSORS,
        ..world.task(Mode::NoGamepad)
    });
    let mut handler = Handler::new(VERSION, stock_identity(None)).with_sensors(&SENSORS);
    let released = read(&mut handler, &mut bench, Reading::Value).unwrap();
    assert_eq!(send::<SetRawScanning>(&mut handler, &true), Some(Ok(())));
    assert!(SENSORS.raw_scanning());
    bench.cycle();
    bench.press(KEY_S, FULL);
    let pressed = read(&mut handler, &mut bench, Reading::Value).unwrap();
    assert!(value(&pressed, KEY_S).unwrap() > value(&released, KEY_S).unwrap());
    let travel = read(&mut handler, &mut bench, Reading::Travel).unwrap();
    assert_eq!(value(&travel, KEY_S), Some(0));
    assert_eq!(send::<SetRawScanning>(&mut handler, &false), Some(Ok(())));
    bench.cycle();
    let travel = read(&mut handler, &mut bench, Reading::Travel).unwrap();
    assert!(value(&travel, KEY_S).unwrap() > 3000, "{travel:?}");
}

#[test]
fn a_handler_without_the_sensor_readout_rejects_the_request() {
    let mut handler = Handler::new(VERSION, stock_identity(None));
    assert_eq!(
        send::<GetSensorValues>(&mut handler, &Reading::Travel),
        Some(Err(Rejection::Unsupported))
    );
    assert_eq!(
        send::<SetRawScanning>(&mut handler, &true),
        Some(Err(Rejection::Unsupported))
    );
}
