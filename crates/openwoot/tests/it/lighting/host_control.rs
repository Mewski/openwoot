use openwoot::{
    keys::Key,
    lighting::{BLACK, HostControl, Lightbar, from_rgb565, rgb},
    protocol::Handler,
};
use openwoot_protocol::{
    configuration::{Invalid, Rejection},
    endpoints::{
        GetLedFrame, GetLightbarColors, GetProfileState, LeaveHostControl, ResetHostColors,
        ResetHostKeyColor, ResetHostLightbar, SetHostColors, SetHostKeyColor, SetHostLightbar,
    },
    host_lighting::KeyColor,
    keys::Position,
    lightbar,
    lighting::{Effects, KeyColors, Rgb},
};

use super::support::{
    BAR_LEDS, BarLighting, COLUMNS, RED, ROWS, START_MS, TestLighting, VERSION, ask, base_color,
    connect, lighting, lit_lighting, read, red_bar,
};
use crate::support::{KeyMatrix, stock_identity};

const KEY: Key = Key::new(2, 3);
const POSITION: Position = Position { row: 2, column: 3 };

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

fn bar_of(colors: &[Rgb]) -> lightbar::Colors {
    (0..BAR_LEDS.len())
        .map(|index| colors.get(index).copied().unwrap_or(BLACK))
        .collect()
}

fn shown_lightbar(lighting: &BarLighting) -> lightbar::Colors {
    let mut colors = lightbar::Colors::new();
    lighting.shown_lightbar(&mut colors);
    colors
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
    assert_eq!(
        ask::<GetLedFrame, _>(&mut handler, &()),
        Err(Rejection::Unsupported)
    );
    assert_eq!(
        ask::<GetProfileState, _>(&mut handler, &()),
        Err(Rejection::Unsupported)
    );
}

#[test]
fn the_host_drives_the_light_bar_until_it_resets_or_leaves() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = connect(10, &HOST_CONTROL);
    let mut lighting = lit_lighting(&red_bar(true));
    lighting.update(START_MS + 16, &());
    assert_eq!(shown_lightbar(&lighting), bar_of(&[RED; 10]));

    let green = rgb(0, 255, 0);
    let blue = rgb(0, 0, 255);
    let host: lightbar::Colors = [green, blue].into_iter().collect();
    assert_eq!(ask::<SetHostLightbar, _>(&mut handler, &host), Ok(()));
    HOST_CONTROL.apply(&mut lighting, START_MS + 17);
    assert!(lighting.host_lightbar());
    assert!(!lighting.host_control());
    lighting.update(START_MS + 32, &());
    assert_eq!(
        read::<GetLightbarColors, _, _>(&mut handler, &HOST_CONTROL, &mut lighting, START_MS + 33),
        Ok(bar_of(&[green, blue]))
    );
    assert_eq!(lighting.key_color(KEY), Some(from_rgb565(base_color(2, 3))));

    let magenta = rgb(200, 0, 200);
    let keys: KeyColors = core::iter::repeat_n(magenta, ROWS * COLUMNS).collect();
    assert_eq!(ask::<SetHostColors, _>(&mut handler, &keys), Ok(()));
    assert_eq!(ask::<ResetHostLightbar, _>(&mut handler, &()), Ok(()));
    HOST_CONTROL.apply(&mut lighting, START_MS + 34);
    lighting.update(START_MS + 48, &());
    assert!(!lighting.host_lightbar());
    assert!(lighting.host_control());
    assert_eq!(shown_lightbar(&lighting), bar_of(&[RED; 10]));
    assert_eq!(lighting.key_color(KEY), Some(magenta));

    assert_eq!(ask::<SetHostLightbar, _>(&mut handler, &host), Ok(()));
    assert_eq!(ask::<LeaveHostControl, _>(&mut handler, &()), Ok(()));
    HOST_CONTROL.apply(&mut lighting, START_MS + 49);
    lighting.update(START_MS + 64, &());
    assert!(!lighting.host_lightbar());
    assert!(!lighting.host_control());
    assert_eq!(shown_lightbar(&lighting), bar_of(&[RED; 10]));
    assert_eq!(lighting.key_color(KEY), Some(from_rgb565(base_color(2, 3))));
}

#[test]
fn a_light_bar_the_profile_turns_off_shows_host_colours_at_the_key_brightness() {
    let mut lighting = lit_lighting(&red_bar(false));
    lighting.update(START_MS + 16, &());
    assert_eq!(lighting.hooks().brightness(), 0);
    assert_eq!(shown_lightbar(&lighting), bar_of(&[]));

    lighting.set_host_lightbar(&[RED]);
    assert_eq!(lighting.hooks().brightness(), 255);
    lighting.update(START_MS + 32, &());
    assert_eq!(shown_lightbar(&lighting), bar_of(&[RED]));

    lighting.reset_host_lightbar();
    assert_eq!(lighting.hooks().brightness(), 0);
    lighting.update(START_MS + 48, &());
    assert_eq!(shown_lightbar(&lighting), bar_of(&[]));
}

#[test]
fn light_bar_colours_beyond_its_leds_or_without_a_light_bar_are_refused() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = connect(10, &HOST_CONTROL);
    let eleven: lightbar::Colors = core::iter::repeat_n(RED, 11).collect();
    assert_eq!(
        ask::<SetHostLightbar, _>(&mut handler, &eleven),
        Err(Rejection::Invalid(Invalid::LightbarColorsExceedLeds {
            count: 11,
            leds: 10
        }))
    );
    let mut without = connect(0, &HOST_CONTROL);
    let one: lightbar::Colors = [RED].into_iter().collect();
    assert_eq!(
        ask::<SetHostLightbar, _>(&mut without, &one),
        Err(Rejection::Invalid(Invalid::NoLightbar))
    );
    assert_eq!(
        ask::<ResetHostLightbar, _>(&mut without, &()),
        Err(Rejection::Unsupported)
    );
    assert_eq!(
        ask::<GetLightbarColors, _>(&mut without, &()),
        Err(Rejection::Unsupported)
    );
    let mut lighting = lit_lighting(&red_bar(true));
    HOST_CONTROL.apply(&mut lighting, START_MS);
    assert!(!lighting.host_lightbar());
}
