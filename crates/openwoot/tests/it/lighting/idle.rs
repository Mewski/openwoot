use openwoot::{
    keys::Key,
    lighting::{FADE_DIMMED, FADE_FULL, KeyEvent},
};
use openwoot_protocol::lighting::{Effects, Idle, Rgb};

use super::support::{Chip, START_MS, lighting, settle};

#[test]
fn idle_dims_then_switches_off_and_a_key_press_wakes_the_leds() {
    let mut lighting = lighting(255, Effects::new());
    lighting.set_idle(Some(Idle {
        dim_after_minutes: 1,
        off_after_minutes: 2,
        off_enabled: true,
    }));
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 59_000);
    assert_eq!(lighting.fade_level(), FADE_FULL);
    let bright = chip.key(2, 2);
    settle(
        &mut lighting,
        &mut chip,
        START_MS + 59_001,
        START_MS + 63_000,
    );
    assert_eq!(lighting.fade_level(), FADE_DIMMED);
    let dimmed = chip.key(2, 2);
    assert!(dimmed.green < bright.green, "{dimmed:?} {bright:?}");
    settle(
        &mut lighting,
        &mut chip,
        START_MS + 63_001,
        START_MS + 124_000,
    );
    assert!(lighting.is_idle_off());
    assert_eq!(lighting.fade_level(), 0);
    assert_eq!(chip.key(2, 2), Rgb::default());
    lighting.key_event(Key::new(2, 2), KeyEvent::Pressed, START_MS + 124_001);
    lighting.reset_idle_timer(START_MS + 124_001);
    settle(
        &mut lighting,
        &mut chip,
        START_MS + 124_001,
        START_MS + 125_000,
    );
    assert_eq!(lighting.fade_level(), FADE_FULL);
    assert!(!lighting.is_idle_off());
}

#[test]
fn idle_without_switching_off_stays_dimmed() {
    let mut lighting = lighting(255, Effects::new());
    lighting.set_idle(Some(Idle {
        dim_after_minutes: 1,
        off_after_minutes: 1,
        off_enabled: false,
    }));
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 200_000);
    assert_eq!(lighting.fade_level(), FADE_DIMMED);
    assert!(!lighting.is_idle_off());
}
