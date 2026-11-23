use openwoot::{
    keys::Key,
    lighting::{FADE_DIMMED, FADE_FULL, KeyEvent, TRAVEL_FULL, rgb},
};
use openwoot_protocol::lighting::{Effects, Idle, Rgb};

use super::support::{Chip, START_MS, TestLighting, effect, lighting, settle};

const IDLE: Idle = Idle {
    dim_after_minutes: 1,
    off_after_minutes: 2,
    off_enabled: true,
};

fn levels(lighting: &mut TestLighting, chip: &mut Chip, from_ms: u32, to_ms: u32) -> (u8, u8) {
    let mut low = u8::MAX;
    let mut high = 0;
    for now_ms in from_ms..=to_ms {
        lighting.update(now_ms, &());
        chip.drain(lighting);
        low = low.min(lighting.fade_level());
        high = high.max(lighting.fade_level());
    }
    (low, high)
}

fn rainbow() -> Effects {
    let mut effects = Effects::new();
    effects.push(effect(1, [0, 20, 1, 0])).unwrap();
    effects
}

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

#[test]
fn host_colours_set_while_the_leds_are_idle_off_are_shown() {
    let mut lighting = lighting(255, Effects::new());
    lighting.set_idle(Some(IDLE));
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 124_000);
    assert!(lighting.is_idle_off());
    let entered = START_MS + 124_001;
    lighting.set_host_key_color(Key::new(2, 2), rgb(200, 0, 200), entered);
    settle(&mut lighting, &mut chip, entered, entered + 1000);
    assert!(!lighting.is_idle_off());
    assert_eq!(lighting.fade_level(), FADE_FULL);
    assert_ne!(chip.key(2, 2), Rgb::default());
    let left = entered + 1001;
    lighting.leave_host_control(left);
    settle(&mut lighting, &mut chip, left, left + 59_000);
    assert_eq!(lighting.fade_level(), FADE_FULL);
    settle(&mut lighting, &mut chip, left + 59_001, left + 63_000);
    assert_eq!(lighting.fade_level(), FADE_DIMMED);
}

#[test]
fn idle_time_in_host_control_does_not_freeze_the_effects_after_leaving() {
    let mut lighting = lighting(255, rainbow());
    lighting.set_idle(Some(IDLE));
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 1000);
    lighting.enter_host_control(START_MS + 1001);
    settle(
        &mut lighting,
        &mut chip,
        START_MS + 1001,
        START_MS + 130_000,
    );
    assert!(!lighting.is_idle_off());
    assert_eq!(lighting.fade_level(), FADE_FULL);
    let left = START_MS + 130_001;
    lighting.leave_host_control(left);
    settle(&mut lighting, &mut chip, left, left + 1000);
    let first = chip.digest();
    settle(&mut lighting, &mut chip, left + 1001, left + 5000);
    assert_ne!(chip.digest(), first);
    assert!(!lighting.is_idle_off());
    assert_eq!(lighting.fade_level(), FADE_FULL);
}

#[test]
fn idle_times_of_zero_minutes_count_as_one_minute() {
    let mut lighting = lighting(255, Effects::new());
    lighting.set_idle(Some(Idle {
        dim_after_minutes: 0,
        off_after_minutes: 0,
        off_enabled: true,
    }));
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 59_000);
    assert_eq!(lighting.fade_level(), FADE_FULL);
    assert!(!lighting.is_idle_off());
    settle(
        &mut lighting,
        &mut chip,
        START_MS + 59_001,
        START_MS + 64_000,
    );
    assert!(lighting.is_idle_off());
    assert_eq!(lighting.fade_level(), 0);
}

#[test]
fn switching_off_before_dimming_fades_from_full_brightness() {
    let mut lighting = lighting(255, Effects::new());
    lighting.set_idle(Some(Idle {
        dim_after_minutes: 3,
        off_after_minutes: 1,
        off_enabled: true,
    }));
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 59_999);
    assert_eq!(lighting.fade_level(), FADE_FULL);
    let (low, _) = levels(
        &mut lighting,
        &mut chip,
        START_MS + 60_000,
        START_MS + 60_100,
    );
    assert!(low > FADE_DIMMED, "jumped to {low}");
    settle(
        &mut lighting,
        &mut chip,
        START_MS + 60_101,
        START_MS + 64_000,
    );
    assert_eq!(lighting.fade_level(), 0);
}

#[test]
fn a_press_during_the_dim_or_off_fade_brightens_from_the_current_level() {
    for fading_ms in [60_300, 120_500] {
        let mut lighting = lighting(255, Effects::new());
        lighting.set_idle(Some(IDLE));
        let mut chip = Chip::new();
        settle(&mut lighting, &mut chip, START_MS, START_MS + fading_ms);
        let current = lighting.fade_level();
        assert!(current < FADE_FULL && current > 0, "{current}");
        let pressed = START_MS + fading_ms + 1;
        lighting.reset_idle_timer(pressed);
        let (low, _) = levels(&mut lighting, &mut chip, pressed, pressed + 100);
        assert!(low >= current, "dipped from {current} to {low}");
        settle(&mut lighting, &mut chip, pressed + 101, pressed + 1000);
        assert_eq!(lighting.fade_level(), FADE_FULL);
    }
}

#[test]
fn a_usb_wake_while_dimmed_returns_to_the_dimmed_level() {
    let mut lighting = lighting(255, Effects::new());
    lighting.set_idle(Some(Idle {
        dim_after_minutes: 1,
        off_after_minutes: 10,
        off_enabled: false,
    }));
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 64_000);
    assert_eq!(lighting.fade_level(), FADE_DIMMED);
    let resumed = START_MS + 64_001;
    lighting.wake(resumed);
    let (_, high) = levels(&mut lighting, &mut chip, resumed, resumed + 2000);
    assert_eq!(high, FADE_DIMMED);
    lighting.reset_idle_timer(resumed + 2001);
    settle(&mut lighting, &mut chip, resumed + 2001, resumed + 3000);
    assert_eq!(lighting.fade_level(), FADE_FULL);
}

#[test]
fn a_held_key_keeps_the_leds_awake_until_no_key_travels() {
    let mut lighting = lighting(255, Effects::new());
    lighting.set_idle(Some(IDLE));
    let mut chip = Chip::new();
    lighting.key_event(Key::new(2, 2), KeyEvent::Pressed, START_MS);
    for now_ms in START_MS..START_MS + 70_000 {
        lighting.hold_idle_timer(TRAVEL_FULL, now_ms);
        lighting.update(now_ms, &());
        chip.drain(&mut lighting);
    }
    assert_eq!(lighting.fade_level(), FADE_FULL);
    for now_ms in START_MS + 70_000..START_MS + 133_000 {
        lighting.hold_idle_timer(0, now_ms);
        lighting.update(now_ms, &());
        chip.drain(&mut lighting);
    }
    assert_eq!(lighting.fade_level(), FADE_DIMMED);
}
