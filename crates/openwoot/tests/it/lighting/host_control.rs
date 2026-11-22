use embassy_futures::block_on;
use openwoot::{
    keys::Key,
    lighting::{HostControl, from_rgb565, rgb},
    protocol::{Configuration, Handler},
};
use openwoot_protocol::{
    Endpoint, FirmwareVersion, PROTOCOL_VERSION, Version,
    configuration::{Invalid, Rejection},
    decode_response, encode_request,
    endpoints::{
        LeaveHostControl, ResetHostColors, ResetHostKeyColor, SetHostColors, SetHostKeyColor,
    },
    host_lighting::KeyColor,
    keys::Position,
    lighting::{Effects, KeyColors},
    report,
};
use serde::{Serialize, de::DeserializeOwned};

use super::support::{COLUMNS, ROWS, TestLighting, base_color, lighting};
use crate::support::{KeyMatrix, stock_identity};

const VERSION: Version = Version {
    protocol: PROTOCOL_VERSION,
    firmware: FirmwareVersion {
        major: 0,
        minor: 1,
        patch: 0,
    },
};
const KEY: Key = Key::new(2, 3);
const POSITION: Position = Position { row: 2, column: 3 };

fn ask<E, C>(handler: &mut Handler<C>, request: &E::Request) -> E::Response
where
    E: Endpoint,
    E::Request: Serialize,
    E::Response: DeserializeOwned,
    C: Configuration,
{
    let mut encoded = [0; report::MAX_LEN];
    let encoded = encode_request::<E>(7, request, &mut encoded).unwrap();
    let mut response = [0; report::MAX_LEN];
    let answer = block_on(handler.respond(encoded, &mut response));
    decode_response::<E>(7, answer.reply.unwrap())
        .unwrap()
        .unwrap()
}

fn mirrored(row: usize, column: usize) -> u16 {
    base_color(ROWS - 1 - row, COLUMNS - 1 - column)
}

fn keys_showing(lighting: &TestLighting, colors: impl Fn(usize, usize) -> u16) -> (usize, usize) {
    let mut with_leds = 0;
    let mut showing = 0;
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let key = Key::new(u8::try_from(row).unwrap(), u8::try_from(column).unwrap());
            let Some(color) = lighting.key_color(key) else {
                continue;
            };
            with_leds += 1;
            if color == from_rgb565(colors(row, column)) {
                showing += 1;
            }
        }
    }
    (with_leds, showing)
}

#[test]
fn the_host_drives_single_keys_and_whole_maps_until_it_leaves() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = Handler::with_configuration(VERSION, stock_identity(None), KeyMatrix)
        .with_host_control(&HOST_CONTROL);
    let mut lighting = lighting(255, Effects::new());
    let magenta = rgb(200, 0, 200);
    let key_color = KeyColor {
        key: POSITION,
        color: magenta,
    };
    assert_eq!(ask::<SetHostKeyColor, _>(&mut handler, &key_color), Ok(()));
    HOST_CONTROL.apply(&mut lighting, 0);
    assert!(lighting.host_control());
    assert_eq!(lighting.key_color(KEY), Some(magenta));

    let colors: KeyColors = (0..ROWS)
        .flat_map(|row| (0..COLUMNS).map(move |column| from_rgb565(mirrored(row, column))))
        .collect();
    assert_eq!(ask::<SetHostColors, _>(&mut handler, &colors), Ok(()));
    assert_eq!(ask::<ResetHostKeyColor, _>(&mut handler, &POSITION), Ok(()));
    HOST_CONTROL.apply(&mut lighting, 0);
    assert_eq!(lighting.key_color(KEY), Some(from_rgb565(base_color(2, 3))));
    let (with_leds, mirrored_keys) = keys_showing(&lighting, mirrored);
    assert_eq!(mirrored_keys, with_leds - 1);

    assert_eq!(ask::<ResetHostColors, _>(&mut handler, &()), Ok(()));
    HOST_CONTROL.apply(&mut lighting, 0);
    assert!(lighting.host_control());
    assert_eq!(keys_showing(&lighting, base_color), (with_leds, with_leds));

    assert_eq!(ask::<LeaveHostControl, _>(&mut handler, &()), Ok(()));
    HOST_CONTROL.apply(&mut lighting, 0);
    assert!(!lighting.host_control());
}

#[test]
fn keys_and_maps_outside_the_key_matrix_are_rejected() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = Handler::with_configuration(VERSION, stock_identity(None), KeyMatrix)
        .with_host_control(&HOST_CONTROL);
    let outside = Position {
        row: u8::try_from(ROWS).unwrap(),
        column: 0,
    };
    assert_eq!(
        ask::<ResetHostKeyColor, _>(&mut handler, &outside),
        Err(Rejection::Invalid(Invalid::PositionOutsideMatrix {
            row: outside.row,
            column: 0
        }))
    );
    let short: KeyColors = core::iter::repeat_n(rgb(1, 2, 3), 3).collect();
    assert_eq!(
        ask::<SetHostColors, _>(&mut handler, &short),
        Err(Rejection::Invalid(Invalid::KeyCountDiffersFromMatrix {
            expected: u16::try_from(ROWS * COLUMNS).unwrap(),
            found: 3
        }))
    );
    let mut lighting = lighting(255, Effects::new());
    HOST_CONTROL.apply(&mut lighting, 0);
    assert!(!lighting.host_control());
}

#[test]
fn a_handler_without_host_control_rejects_led_requests() {
    let mut handler = Handler::with_configuration(VERSION, stock_identity(None), KeyMatrix);
    assert_eq!(
        ask::<LeaveHostControl, _>(&mut handler, &()),
        Err(Rejection::Unsupported)
    );
    assert_eq!(
        ask::<ResetHostKeyColor, _>(&mut handler, &POSITION),
        Err(Rejection::Unsupported)
    );
}
