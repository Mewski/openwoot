use openwoot::lighting::{Indicator, Key, LockState, OverlayId, from_rgb565, rgb};
use openwoot_protocol::lighting::Effects;

use super::{
    base_color,
    engine::{START_MS, lighting},
};

#[test]
fn num_lock_indicator_is_lit_while_num_lock_is_off_unless_not_inverted() {
    let mut lighting = lighting(255, Effects::new());
    let num_lock = Key::new(1, 15);
    lighting.set_indicator_key(Indicator::NumLock, Some(num_lock), START_MS);
    lighting.update_lock_indicators(LockState::default(), START_MS);
    assert!(lighting.overlay_running(OverlayId::NumLock));
    lighting.set_num_lock_inverted(false, START_MS);
    assert!(!lighting.overlay_running(OverlayId::NumLock));
    lighting.update_lock_indicators(
        LockState {
            num_lock: true,
            ..LockState::default()
        },
        START_MS,
    );
    assert!(lighting.overlay_running(OverlayId::NumLock));
}

#[test]
fn host_control_hides_the_host_indicators_and_drives_keys_directly() {
    let mut lighting = lighting(255, Effects::new());
    let caps = Key::new(3, 0);
    lighting.set_indicator_key(Indicator::CapsLock, Some(caps), START_MS);
    lighting.update_lock_indicators(
        LockState {
            caps_lock: true,
            ..LockState::default()
        },
        START_MS,
    );
    assert!(lighting.overlay_running(OverlayId::CapsLock));
    let magenta = rgb(200, 0, 200);
    lighting.set_host_key_color(Key::new(0, 0), magenta, START_MS);
    assert!(lighting.host_control());
    assert!(!lighting.overlay_running(OverlayId::CapsLock));
    assert_eq!(lighting.key_color(Key::new(0, 0)), Some(magenta));
    lighting.reset_host_key_color(Key::new(0, 0));
    assert_eq!(
        lighting.key_color(Key::new(0, 0)),
        Some(from_rgb565(base_color(0, 0)))
    );
    lighting.leave_host_control(START_MS);
    assert!(lighting.overlay_running(OverlayId::CapsLock));
}
