use openwoot::{
    keys::Key,
    lighting::{
        BLACK, Canvas, FADE_DIMMED, Indicator, Lightbar, Lighting, LockState, Overlay, OverlayId,
        Signals, Span, from_rgb565, rgb,
    },
};
use openwoot_protocol::lighting::{Effects, Idle, KeyColors, Rgb};

use super::support::{
    COLUMNS, Chip, GEOMETRY, LENGTH, MAP, PAGES, ROWS, START_MS, TestLighting, VARIANT, base_color,
    lighting, settle,
};

const KEY_OVERLAYS: u8 = 8;

fn caps_lock_on(lighting: &mut TestLighting, caps: Key, now_ms: u32) {
    lighting.set_indicator_key(Indicator::CapsLock, Some(caps), now_ms);
    lighting.update_lock_indicators(
        LockState {
            caps_lock: true,
            ..LockState::default()
        },
        now_ms,
    );
}

fn disable(lighting: &mut TestLighting, key: Key, now_ms: u32) {
    lighting.start_overlay(
        OverlayId::Key(key),
        Span::Forever,
        Overlay::Solid { key, color: BLACK },
        now_ms,
    );
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "compared with `Lighting::key_color`, which answers an `Option`"
)]
fn base(key: Key) -> Option<Rgb> {
    Some(from_rgb565(base_color(key.row(), key.column())))
}

#[test]
fn key_overlays_leave_room_for_the_indicators_and_the_idle_fades() {
    let mut lighting = lighting(255, Effects::new());
    lighting.set_idle(Some(Idle {
        dim_after_minutes: 1,
        off_after_minutes: 2,
        off_enabled: true,
    }));
    for column in 0..KEY_OVERLAYS {
        disable(&mut lighting, Key::new(1, column), START_MS);
        assert!(lighting.overlay_running(OverlayId::Key(Key::new(1, column))));
    }
    let refused = Key::new(1, KEY_OVERLAYS);
    disable(&mut lighting, refused, START_MS);
    assert!(!lighting.overlay_running(OverlayId::Key(refused)));
    caps_lock_on(&mut lighting, Key::new(3, 0), START_MS);
    assert!(lighting.overlay_running(OverlayId::CapsLock));
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 70_000);
    assert_eq!(lighting.fade_level(), FADE_DIMMED);
    settle(
        &mut lighting,
        &mut chip,
        START_MS + 70_001,
        START_MS + 130_000,
    );
    assert!(lighting.is_idle_off());
    assert_eq!(lighting.fade_level(), 0);
    assert_eq!(chip.key(2, 2), Rgb::default());
}

#[test]
fn new_base_colours_keep_the_keys_of_solid_overlays() {
    let mut lighting = lighting(255, Effects::new());
    let mut chip = Chip::new();
    let caps = Key::new(3, 0);
    let disabled = Key::new(1, 1);
    caps_lock_on(&mut lighting, caps, START_MS);
    disable(&mut lighting, disabled, START_MS);
    settle(&mut lighting, &mut chip, START_MS, START_MS + 100);
    let indicator = lighting.key_color(caps);
    assert_ne!(indicator, base(caps));
    let colors: KeyColors = core::iter::repeat_n(rgb(10, 20, 30), ROWS * COLUMNS).collect();
    lighting.set_base_colors(&colors);
    settle(&mut lighting, &mut chip, START_MS + 101, START_MS + 300);
    assert_eq!(lighting.key_color(Key::new(2, 2)), Some(rgb(10, 20, 30)));
    assert_eq!(lighting.key_color(disabled), Some(BLACK));
    assert_eq!(lighting.key_color(caps), indicator);
}

#[test]
fn entering_host_control_gives_the_indicator_key_its_colour_back() {
    let mut lighting = lighting(255, Effects::new());
    let mut chip = Chip::new();
    let caps = Key::new(3, 0);
    caps_lock_on(&mut lighting, caps, START_MS);
    settle(&mut lighting, &mut chip, START_MS, START_MS + 100);
    assert_ne!(lighting.key_color(caps), base(caps));
    lighting.enter_host_control(START_MS + 101);
    assert_eq!(lighting.key_color(caps), base(caps));
    settle(&mut lighting, &mut chip, START_MS + 101, START_MS + 2000);
    assert_eq!(lighting.key_color(caps), base(caps));
    assert!(!lighting.overlay_running(OverlayId::CapsLock));
}

#[derive(Default)]
struct Counter {
    renders: u32,
}

impl Lightbar for Counter {
    fn render(&mut self, _: &mut impl Canvas, _: u16) {
        self.renders += 1;
    }
}

impl Signals for Counter {}

type Counted = Lighting<Counter, ROWS, COLUMNS, PAGES, LENGTH>;

fn renders(lighting: &mut Counted, from_ms: u32, to_ms: u32) -> u32 {
    let before = lighting.hooks().renders;
    for now_ms in from_ms..to_ms {
        lighting.update(now_ms, &());
        while lighting.next_write().is_some() {}
    }
    lighting.hooks().renders - before
}

#[test]
fn stopped_or_new_overlays_do_not_render_every_frame_in_host_control() {
    let mut lighting = Counted::new(&MAP, &VARIANT, GEOMETRY, Counter::default(), START_MS);
    let caps = Key::new(3, 0);
    let disabled = Key::new(1, 1);
    lighting.set_indicator_key(Indicator::CapsLock, Some(caps), START_MS);
    lighting.update_lock_indicators(
        LockState {
            caps_lock: true,
            ..LockState::default()
        },
        START_MS,
    );
    lighting.start_overlay(
        OverlayId::Key(disabled),
        Span::Forever,
        Overlay::Solid {
            key: disabled,
            color: BLACK,
        },
        START_MS,
    );
    renders(&mut lighting, START_MS, START_MS + 1000);
    assert_eq!(renders(&mut lighting, START_MS + 1000, START_MS + 2000), 0);
    lighting.enter_host_control(START_MS + 2000);
    lighting.stop_overlay(OverlayId::Key(disabled));
    let toggled = Key::new(2, 2);
    lighting.start_overlay(
        OverlayId::Key(toggled),
        Span::Forever,
        Overlay::Solid {
            key: toggled,
            color: rgb(255, 255, 255),
        },
        START_MS + 2000,
    );
    renders(&mut lighting, START_MS + 2000, START_MS + 2100);
    assert_eq!(renders(&mut lighting, START_MS + 2100, START_MS + 4100), 0);
    lighting.leave_host_control(START_MS + 4100);
    renders(&mut lighting, START_MS + 4100, START_MS + 4200);
    assert!(!lighting.overlay_running(OverlayId::Key(disabled)));
    assert!(lighting.overlay_running(OverlayId::Key(toggled)));
    assert_eq!(lighting.key_color(toggled), Some(rgb(255, 255, 255)));
}

#[test]
fn moving_an_indicator_key_gives_the_old_key_its_colour_back() {
    let mut lighting = lighting(255, Effects::new());
    let old = Key::new(3, 0);
    let new = Key::new(2, 0);
    let mut chip = Chip::new();
    caps_lock_on(&mut lighting, old, START_MS);
    settle(&mut lighting, &mut chip, START_MS, START_MS + 1000);
    let lit = lighting.key_color(old);
    assert_ne!(lit, base(old));
    lighting.set_indicator_key(Indicator::CapsLock, Some(new), START_MS + 1001);
    settle(&mut lighting, &mut chip, START_MS + 1001, START_MS + 2000);
    assert_eq!(lighting.key_color(old), base(old));
    assert_eq!(lighting.key_color(new), lit);
}

#[test]
fn a_timed_solid_overlay_ends_on_a_still_picture() {
    let mut lighting = lighting(255, Effects::new());
    let key = Key::new(2, 2);
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 100);
    lighting.start_overlay(
        OverlayId::Key(key),
        Span::Milliseconds(100),
        Overlay::Solid {
            key,
            color: rgb(255, 255, 255),
        },
        START_MS + 101,
    );
    settle(&mut lighting, &mut chip, START_MS + 101, START_MS + 150);
    assert_eq!(lighting.key_color(key), Some(rgb(255, 255, 255)));
    settle(&mut lighting, &mut chip, START_MS + 151, START_MS + 1000);
    assert!(!lighting.overlay_running(OverlayId::Key(key)));
    assert_eq!(lighting.key_color(key), base(key));
}
