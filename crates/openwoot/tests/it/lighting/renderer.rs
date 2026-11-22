use openwoot::lighting::{
    FADE_DIMMED, FADE_FULL, Indicator, Key, KeyEvent, Lighting, LockState, Overlay, OverlayId,
    Span, from_rgb565, rgb,
};
use openwoot_protocol::lighting::{Color, Effects, Idle, Indicators, KeyColors, Rgb, Settings};

use super::{
    COLUMNS, GEOMETRY, LENGTH, MAP, PAGES, ROWS, VARIANT, base_color,
    effects::{effect, unpack},
};

const GOLDEN: &str = include_str!("outputs.txt");
const START_MS: u32 = 1000;

type TestLighting = Lighting<(), ROWS, COLUMNS, PAGES, LENGTH>;

struct Chip {
    registers: [[u8; 256]; 2],
}

impl Chip {
    fn new() -> Self {
        Self {
            registers: [[0; 256]; 2],
        }
    }

    fn drain(&mut self, lighting: &mut TestLighting) -> usize {
        let mut writes = 0;
        while let Some(write) = lighting.next_write() {
            let bank = usize::from(write.bank);
            let first = usize::from(write.first_register);
            self.registers[bank][first..first + write.values().len()]
                .copy_from_slice(write.values());
            writes += 1;
        }
        writes
    }

    fn key(&self, row: usize, column: usize) -> Rgb {
        let Some(position) = VARIANT.led(row, column).and_then(|led| MAP.position(led)) else {
            return Rgb::default();
        };
        let page = MAP.page(usize::from(position.page)).unwrap();
        let register = usize::from(page.first_register) + usize::from(position.offset);
        let channels = &self.registers[usize::from(page.bank)][register..register + 3];
        rgb(channels[0], channels[1], channels[2])
    }

    fn digest(&self) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                let color = self.key(row, column);
                for byte in [color.red, color.green, color.blue] {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
                }
            }
        }
        hash
    }
}

fn key_colors(color: impl Fn(usize, usize) -> u16) -> KeyColors {
    let mut colors = KeyColors::new();
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            colors.push(from_rgb565(color(row, column))).unwrap();
        }
    }
    colors
}

fn white() -> Color {
    Color {
        red: 255,
        green: 255,
        blue: 255,
        alpha: 255,
    }
}

fn settings(brightness: u8, effects: Effects) -> Settings {
    Settings {
        brightness,
        indicators: Indicators {
            caps_lock: unpack(0x80FF_2000),
            scroll_lock: white(),
            num_lock: white(),
            layer_lock: white(),
        },
        effects,
    }
}

fn lighting(brightness: u8, effects: Effects) -> TestLighting {
    let mut lighting = Lighting::new(&MAP, &VARIANT, GEOMETRY, (), START_MS);
    lighting.apply_settings(&settings(brightness, effects), START_MS);
    lighting.set_base_colors(&key_colors(base_color));
    lighting.set_layer_colors(
        1,
        Some(&key_colors(|row, column| base_color(column, row + 3))),
    );
    lighting
}

fn key(row: &str, column: &str) -> Key {
    Key::new(row.parse().unwrap(), column.parse().unwrap())
}

fn color(words: &[&str]) -> Rgb {
    rgb(
        words[0].parse().unwrap(),
        words[1].parse().unwrap(),
        words[2].parse().unwrap(),
    )
}

fn apply(lighting: &mut TestLighting, operation: &[&str], now_ms: u32) {
    match operation {
        ["W"] => lighting.wake(now_ms),
        ["K", row, column, rest @ ..] => {
            let key = key(row, column);
            lighting.start_overlay(
                OverlayId::Key(key),
                Span::Forever,
                Overlay::Solid {
                    key,
                    color: color(rest),
                },
                now_ms,
            );
        }
        ["X", row, column] => lighting.stop_overlay(OverlayId::Key(key(row, column))),
        ["F", row, column, rest @ ..] => lighting.start_overlay(
            OverlayId::ProfileSwitch,
            Span::Milliseconds(1000),
            Overlay::Blink {
                key: key(row, column),
                first: color(&rest[..3]),
                second: color(&rest[3..6]),
                period_ms: rest[6].parse().unwrap(),
            },
            now_ms,
        ),
        ["U"] => {
            lighting.step_brightness(true, now_ms);
        }
        ["D"] => {
            lighting.step_brightness(false, now_ms);
        }
        ["C", row, column, on] => {
            lighting.set_indicator_key(Indicator::CapsLock, Some(key(row, column)), now_ms);
            lighting.update_lock_indicators(
                LockState {
                    caps_lock: *on == "1",
                    num_lock: true,
                    ..LockState::default()
                },
                now_ms,
            );
        }
        ["L", layer] => lighting.show_layer(layer.parse().unwrap()),
        ["P", row, column] => {
            lighting.key_event(key(row, column), KeyEvent::Pressed, now_ms);
            lighting.reset_idle_timer(now_ms);
        }
        other => panic!("unknown operation {other:?} in the golden data"),
    }
}

fn run_scenario(line: &str) -> (String, Vec<u64>) {
    let words: Vec<&str> = line.split_whitespace().skip(1).collect();
    let name = words[0].to_owned();
    let duration: u32 = words[1].parse().unwrap();
    let brightness: u8 = words[2].parse().unwrap();
    let mut effects = Effects::new();
    let mut operations: Vec<(u32, Vec<&str>)> = Vec::new();
    let mut index = 3;
    while index < words.len() {
        match words[index] {
            "L" => {
                let number = |offset: usize| words[index + offset].parse::<u32>().unwrap();
                effects
                    .push(effect(
                        number(1),
                        [number(2), number(3), number(4), number(5)],
                    ))
                    .unwrap();
                index += 6;
            }
            "O" => {
                let time = words[index + 1].parse().unwrap();
                let end = words[index + 2..]
                    .iter()
                    .position(|word| *word == "O")
                    .map_or(words.len(), |offset| index + 2 + offset);
                operations.push((time, words[index + 2..end].to_vec()));
                index = end;
            }
            other => panic!("unknown tag `{other}` in the golden data"),
        }
    }
    let mut lighting = lighting(brightness, effects);
    let mut chip = Chip::new();
    let mut digests = Vec::new();
    for now_ms in START_MS..=START_MS + duration {
        for (_, operation) in operations.iter().filter(|(time, _)| *time == now_ms) {
            apply(&mut lighting, operation, now_ms);
        }
        lighting.update(now_ms, &());
        chip.drain(&mut lighting);
        if (now_ms - START_MS) % 16 == 15 {
            digests.push(chip.digest());
        }
    }
    (name, digests)
}

#[test]
fn outputs_match_the_reference_firmware_through_fades_overlays_and_layers() {
    let expected: Vec<(String, Vec<u64>)> = GOLDEN
        .lines()
        .filter_map(|line| line.strip_prefix("g "))
        .map(|line| {
            let mut words = line.split_whitespace();
            let name = words.next().unwrap().to_owned();
            let digests = words
                .map(|digest| u64::from_str_radix(digest, 16).unwrap())
                .collect();
            (name, digests)
        })
        .collect();
    let mut failures = Vec::new();
    for line in GOLDEN.lines().filter(|line| line.starts_with("e ")) {
        let (name, digests) = run_scenario(line);
        let wanted = &expected
            .iter()
            .find(|(wanted, _)| *wanted == name)
            .unwrap()
            .1;
        assert_eq!(digests.len(), wanted.len(), "{name}");
        if let Some(checkpoint) = digests
            .iter()
            .zip(wanted)
            .position(|(actual, wanted)| actual != wanted)
        {
            failures.push(format!("{name} differs at checkpoint {checkpoint}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

fn settle(lighting: &mut TestLighting, chip: &mut Chip, from_ms: u32, to_ms: u32) {
    for now_ms in from_ms..=to_ms {
        lighting.update(now_ms, &());
        chip.drain(lighting);
    }
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
fn brightness_steps_stop_at_the_limits_and_request_one_save() {
    let mut lighting = lighting(250, Effects::new());
    assert_eq!(lighting.step_brightness(true, START_MS), 255);
    assert_eq!(lighting.step_brightness(true, START_MS + 10), 255);
    assert_eq!(lighting.take_brightness_save(START_MS + 1000), None);
    assert_eq!(lighting.take_brightness_save(START_MS + 2010), Some(255));
    assert_eq!(lighting.take_brightness_save(START_MS + 5000), None);
    lighting.set_fast_scan(true);
    assert_eq!(lighting.step_brightness(true, START_MS), 179);
    let mut dark = self::lighting(3, Effects::new());
    assert_eq!(dark.step_brightness(false, START_MS), 0);
}

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

#[test]
fn a_blink_with_period_zero_shows_its_first_colour() {
    let mut lighting = lighting(255, Effects::new());
    let mut chip = Chip::new();
    let key = Key::new(1, 1);
    lighting.start_overlay(
        OverlayId::ProfileSwitch,
        Span::Forever,
        Overlay::Blink {
            key,
            first: rgb(255, 0, 0),
            second: rgb(0, 0, 255),
            period_ms: 0,
        },
        START_MS,
    );
    settle(&mut lighting, &mut chip, START_MS, START_MS + 100);
    assert_eq!(lighting.key_color(key), Some(rgb(255, 0, 0)));
}

#[test]
fn a_full_frame_of_six_effect_layers_renders_and_queues_every_changed_page() {
    let mut effects = Effects::new();
    for code in [[1, 0, 4, 1, 0], [6, 40, 4, 1, 0], [3, 15, 30, 0, 0]] {
        effects
            .push(effect(code[0], [code[1], code[2], code[3], code[4]]))
            .unwrap();
    }
    effects.push(effect(14, [63488, 31, 40, 30])).unwrap();
    effects.push(effect(15, [63488, 2016, 30, 50])).unwrap();
    effects.push(effect(5, [1, 20, 30, 40])).unwrap();
    let mut lighting = lighting(255, effects);
    let mut chip = Chip::new();
    lighting.update(START_MS, &());
    chip.drain(&mut lighting);
    let started = std::time::Instant::now();
    let mut writes = 0;
    for frame in 1..=100 {
        let now_ms = START_MS + 16 * frame;
        lighting.key_event(Key::new(2, (frame % 17) as u8), KeyEvent::Pressed, now_ms);
        lighting.update(now_ms, &());
        let frame_writes = chip.drain(&mut lighting);
        assert!(frame_writes <= PAGES);
        writes += frame_writes;
    }
    let per_frame = started.elapsed() / 100;
    assert!(writes > 100);
    assert!(
        per_frame < std::time::Duration::from_millis(16),
        "{per_frame:?}"
    );
}
