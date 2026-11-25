use openwoot::{
    lighting::{BLACK, HostControl, from_rgb565, rgb},
    protocol::Handler,
};
use openwoot_protocol::{
    configuration::Rejection,
    endpoints::{
        GetLedFrame, GetLightbarColors, GetProfileState, LeaveHostControl, ResetHostLightbar,
        SetHostColors, SetHostKeyColor, SetHostLightbar,
    },
    host_lighting::{Frame, KeyColor},
    keys::Position,
    lightbar,
    lighting::{Effects, Idle, KeyColors},
    profile,
};

use super::support::{
    ACTIVE, BarLighting, COLUMNS, Keyboard, RED, ROWS, START_MS, ask, base_color, connect,
    lighting, lit_lighting, read, red_bar,
};

const IDLE: Idle = Idle {
    dim_after_minutes: 1,
    off_after_minutes: 2,
    off_enabled: true,
};

fn state(host_control: bool, idle_off: bool) -> profile::State {
    profile::State {
        active: ACTIVE,
        host_control,
        idle_off,
    }
}

fn frame(
    handler: &mut Handler<Keyboard>,
    host_control: &HostControl,
    lighting: &mut BarLighting,
    now_ms: u32,
) -> Frame {
    read::<GetLedFrame, _, _>(handler, host_control, lighting, now_ms).unwrap()
}

fn profile_state(
    handler: &mut Handler<Keyboard>,
    host_control: &HostControl,
    lighting: &mut BarLighting,
    now_ms: u32,
) -> Result<profile::State, Rejection> {
    host_control.apply(lighting, now_ms);
    ask::<GetProfileState, _>(handler, &())
}

#[test]
fn the_frame_shows_the_colours_the_host_painted_and_the_light_bar() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = connect(10, &HOST_CONTROL);
    let mut lighting = lit_lighting(&red_bar(true));
    lighting.update(START_MS + 16, &());
    let magenta = rgb(200, 0, 200);
    let key_color = KeyColor {
        key: Position { row: 2, column: 3 },
        color: magenta,
    };
    assert_eq!(ask::<SetHostKeyColor, _>(&mut handler, &key_color), Ok(()));
    let shown = frame(&mut handler, &HOST_CONTROL, &mut lighting, START_MS + 17);
    assert_eq!(shown.keys.len(), ROWS * COLUMNS);
    assert_eq!(shown.keys[2 * COLUMNS + 3], magenta);
    assert_eq!(shown.keys[0], from_rgb565(base_color(0, 0)));
    assert_eq!(shown.keys[9], BLACK);
    assert_eq!(shown.lightbar.as_slice(), &[RED; 10]);
}

#[test]
fn every_led_reads_black_while_the_idle_timer_has_switched_them_off() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = connect(10, &HOST_CONTROL);
    let mut lighting = lit_lighting(&red_bar(true));
    lighting.set_idle(Some(IDLE));
    lighting.update(START_MS + 16, &());
    lighting.update(START_MS + 130_000, &());
    assert!(lighting.is_idle_off());
    let dark = frame(
        &mut handler,
        &HOST_CONTROL,
        &mut lighting,
        START_MS + 130_001,
    );
    assert_eq!(dark.keys.len(), ROWS * COLUMNS);
    assert_eq!(dark.lightbar.len(), 10);
    assert!(
        dark.keys
            .iter()
            .chain(&dark.lightbar)
            .all(|color| *color == BLACK)
    );
    let unlit: lightbar::Colors = [BLACK; 10].into_iter().collect();
    assert_eq!(
        read::<GetLightbarColors, _, _>(
            &mut handler,
            &HOST_CONTROL,
            &mut lighting,
            START_MS + 130_002
        ),
        Ok(unlit)
    );
    assert_eq!(
        profile_state(
            &mut handler,
            &HOST_CONTROL,
            &mut lighting,
            START_MS + 130_003
        ),
        Ok(state(false, true))
    );

    lighting.reset_idle_timer(START_MS + 130_004);
    let woken = frame(
        &mut handler,
        &HOST_CONTROL,
        &mut lighting,
        START_MS + 130_005,
    );
    assert_eq!(woken.keys[0], from_rgb565(base_color(0, 0)));
    assert_eq!(woken.lightbar.as_slice(), &[RED; 10]);
    assert_eq!(
        profile_state(
            &mut handler,
            &HOST_CONTROL,
            &mut lighting,
            START_MS + 130_006
        ),
        Ok(state(false, false))
    );
}

#[test]
fn the_profile_state_tells_whether_the_host_drives_the_keys_or_the_light_bar() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = connect(10, &HOST_CONTROL);
    let mut lighting = lit_lighting(&red_bar(true));
    assert_eq!(
        profile_state(&mut handler, &HOST_CONTROL, &mut lighting, START_MS),
        Ok(state(false, false))
    );

    let keys: KeyColors = core::iter::repeat_n(RED, ROWS * COLUMNS).collect();
    assert_eq!(ask::<SetHostColors, _>(&mut handler, &keys), Ok(()));
    assert_eq!(
        profile_state(&mut handler, &HOST_CONTROL, &mut lighting, START_MS + 1),
        Ok(state(true, false))
    );
    assert_eq!(ask::<LeaveHostControl, _>(&mut handler, &()), Ok(()));
    assert_eq!(
        profile_state(&mut handler, &HOST_CONTROL, &mut lighting, START_MS + 2),
        Ok(state(false, false))
    );

    let bar: lightbar::Colors = [RED].into_iter().collect();
    assert_eq!(ask::<SetHostLightbar, _>(&mut handler, &bar), Ok(()));
    assert_eq!(
        profile_state(&mut handler, &HOST_CONTROL, &mut lighting, START_MS + 3),
        Ok(state(true, false))
    );
    assert_eq!(ask::<ResetHostLightbar, _>(&mut handler, &()), Ok(()));
    assert_eq!(
        profile_state(&mut handler, &HOST_CONTROL, &mut lighting, START_MS + 4),
        Ok(state(false, false))
    );
}

#[test]
fn the_profile_state_answers_at_once_unless_host_lighting_requests_are_queued() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = connect(10, &HOST_CONTROL);
    let mut lighting = lit_lighting(&red_bar(true));
    assert_eq!(
        ask::<GetProfileState, _>(&mut handler, &()),
        Ok(state(false, false))
    );
    let keys: KeyColors = core::iter::repeat_n(RED, ROWS * COLUMNS).collect();
    assert_eq!(ask::<SetHostColors, _>(&mut handler, &keys), Ok(()));
    assert_eq!(
        read::<GetProfileState, _, _>(&mut handler, &HOST_CONTROL, &mut lighting, START_MS),
        Ok(state(true, false))
    );
    assert_eq!(
        ask::<GetProfileState, _>(&mut handler, &()),
        Ok(state(true, false))
    );
}

#[test]
fn a_keyboard_without_a_light_bar_reads_its_keys_and_no_light_bar() {
    static HOST_CONTROL: HostControl = HostControl::new();
    let mut handler = connect(0, &HOST_CONTROL);
    let mut lighting = lighting(255, Effects::new());
    let shown =
        read::<GetLedFrame, _, _>(&mut handler, &HOST_CONTROL, &mut lighting, START_MS).unwrap();
    assert_eq!(shown.keys.len(), ROWS * COLUMNS);
    assert_eq!(shown.keys[2 * COLUMNS + 3], from_rgb565(base_color(2, 3)));
    assert!(shown.lightbar.is_empty());
    assert_eq!(
        ask::<GetLightbarColors, _>(&mut handler, &()),
        Err(Rejection::Unsupported)
    );
}
