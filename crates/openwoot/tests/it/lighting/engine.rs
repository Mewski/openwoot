use openwoot::{
    keys::Key,
    lighting::{Indicator, KeyEvent, LockState, Overlay, OverlayId, Span, from_rgb565, rgb},
};
use openwoot_protocol::lighting::{Effects, Rgb};

use super::support::{Chip, PAGES, START_MS, TestLighting, base_color, effect, lighting, settle};

const GOLDEN: &str = include_str!("outputs.txt");
const SCENARIOS: usize = 8;

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
    assert!(
        operations
            .iter()
            .all(|(time, _)| (START_MS..=START_MS + duration).contains(time)),
        "{name} has an operation outside its {duration} ms"
    );
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
    for line in GOLDEN.lines() {
        assert!(
            line.starts_with("e ") || line.starts_with("g "),
            "unknown line `{line}` in the golden data"
        );
    }
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
    let scenarios: Vec<&str> = GOLDEN
        .lines()
        .filter(|line| line.starts_with("e "))
        .collect();
    assert_eq!(scenarios.len(), SCENARIOS);
    assert_eq!(expected.len(), SCENARIOS);
    let mut failures = Vec::new();
    for line in scenarios {
        let (name, digests) = run_scenario(line);
        let mut matching = expected.iter().filter(|(wanted, _)| *wanted == name);
        let wanted = &matching.next().unwrap().1;
        assert!(matching.next().is_none(), "`{name}` has two result lines");
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

#[test]
fn brightness_steps_stop_at_the_limits_and_request_one_save() {
    let mut lighting = lighting(250, Effects::new());
    assert_eq!(lighting.step_brightness(true, START_MS), 255);
    assert_eq!(lighting.step_brightness(true, START_MS + 10), 255);
    assert_eq!(lighting.take_brightness_save(START_MS + 1000), None);
    assert_eq!(lighting.take_brightness_save(START_MS + 2010), Some(255));
    assert_eq!(lighting.take_brightness_save(START_MS + 5000), None);
    lighting.set_fast_scan(true);
    assert_eq!(lighting.step_brightness(true, START_MS), 255);
    let mut capped = self::lighting(170, Effects::new());
    capped.set_fast_scan(true);
    assert_eq!(capped.step_brightness(true, START_MS), 175);
    assert_eq!(capped.step_brightness(true, START_MS), 179);
    assert_eq!(capped.step_brightness(true, START_MS), 179);
    let mut dark = self::lighting(3, Effects::new());
    assert_eq!(dark.step_brightness(false, START_MS), 0);
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
    let mut refreshed = Chip::new();
    lighting.update(START_MS, &());
    chip.drain(&mut lighting);
    let mut writes = 0;
    for frame in 1..=100 {
        let now_ms = START_MS + 16 * frame;
        lighting.key_event(Key::new(2, (frame % 17) as u8), KeyEvent::Pressed, now_ms);
        lighting.update(now_ms, &());
        let frame_writes = chip.drain(&mut lighting);
        assert!(frame_writes <= PAGES);
        writes += frame_writes;
        lighting.invalidate();
        assert_eq!(refreshed.drain(&mut lighting), PAGES);
        assert!(
            chip == refreshed,
            "frame {frame} left a changed page unsent"
        );
    }
    assert!(writes > 100);
}

fn single(kind: u32, parameters: [u32; 4]) -> Effects {
    let mut effects = Effects::new();
    effects.push(effect(kind, parameters)).unwrap();
    effects
}

fn brightness_sum(lighting: &TestLighting, key: Key) -> u32 {
    let color = lighting.key_color(key).unwrap();
    u32::from(color.red) + u32::from(color.green) + u32::from(color.blue)
}

#[test]
fn the_next_frame_falls_due_one_frame_period_after_the_last_render() {
    let mut lighting = lighting(255, single(1, [0, 4, 1, 0]));
    lighting.update(START_MS + 16, &());
    assert_eq!(lighting.until_frame_ms(START_MS + 21), 11);
    lighting.update(START_MS + 31, &());
    assert_eq!(lighting.until_frame_ms(START_MS + 31), 1);
    lighting.update(START_MS + 32, &());
    assert_eq!(lighting.until_frame_ms(START_MS + 32), 16);
}

#[test]
fn a_reactive_press_after_a_quiet_pause_still_flashes() {
    let mut lighting = lighting(255, single(6, [60, 2, 1, 0]));
    let mut chip = Chip::new();
    let key = Key::new(2, 3);
    settle(&mut lighting, &mut chip, START_MS, START_MS + 2000);
    for press in 1..=3 {
        let at = START_MS + 2000 * press;
        lighting.key_event(key, KeyEvent::Pressed, at);
        let mut peak = 0;
        for now_ms in at..at + 2000 {
            lighting.update(now_ms, &());
            chip.drain(&mut lighting);
            peak = peak.max(brightness_sum(&lighting, key));
        }
        assert!(peak >= 254, "press {press} peaks at {peak}");
    }
}

#[test]
fn host_colours_stay_while_a_layer_is_shown_in_host_control() {
    let mut lighting = lighting(255, Effects::new());
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 100);
    let key = Key::new(2, 3);
    let host = rgb(1, 2, 3);
    lighting.set_host_key_color(key, host, START_MS + 101);
    lighting.show_layer(1);
    settle(&mut lighting, &mut chip, START_MS + 101, START_MS + 300);
    assert_eq!(lighting.key_color(key), Some(host));
    lighting.show_layer(0);
    settle(&mut lighting, &mut chip, START_MS + 301, START_MS + 400);
    assert_eq!(lighting.key_color(key), Some(host));
    lighting.show_layer(1);
    lighting.leave_host_control(START_MS + 401);
    settle(&mut lighting, &mut chip, START_MS + 401, START_MS + 500);
    assert_eq!(lighting.key_color(key), Some(from_rgb565(base_color(3, 5))));
}

#[test]
fn resetting_host_colours_outside_host_control_changes_nothing() {
    let mut lighting = lighting(255, single(6, [40, 4, 0, 0]));
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 3000);
    let key = Key::new(2, 3);
    let idle = lighting.key_color(key);
    assert_ne!(idle, Some(from_rgb565(base_color(2, 3))));
    lighting.reset_host_key_color(key);
    lighting.reset_host_colors();
    settle(&mut lighting, &mut chip, START_MS + 3001, START_MS + 5000);
    assert!(!lighting.host_control());
    assert_eq!(lighting.key_color(key), idle);
}

#[test]
fn presses_under_a_shown_layer_do_not_flash_when_it_hides() {
    let mut lighting = lighting(255, single(6, [40, 4, 0, 0]));
    let mut chip = Chip::new();
    settle(&mut lighting, &mut chip, START_MS, START_MS + 3000);
    let keys = [3_u8, 5, 7, 9].map(|column| Key::new(2, column));
    let idle = keys.map(|key| lighting.key_color(key));
    lighting.show_layer(1);
    settle(&mut lighting, &mut chip, START_MS + 3001, START_MS + 3100);
    for (offset, key) in (0..).zip(keys) {
        lighting.key_event(key, KeyEvent::Pressed, START_MS + 3100 + 1000 * offset);
    }
    settle(&mut lighting, &mut chip, START_MS + 3101, START_MS + 9000);
    lighting.show_layer(0);
    settle(&mut lighting, &mut chip, START_MS + 9001, START_MS + 9040);
    assert_eq!(keys.map(|key| lighting.key_color(key)), idle);
}

#[test]
fn the_key_test_still_marks_presses_under_a_shown_layer() {
    let mut lighting = lighting(255, single(11, [40, 0, 0, 0]));
    let mut chip = Chip::new();
    let key = Key::new(2, 2);
    settle(&mut lighting, &mut chip, START_MS, START_MS + 100);
    lighting.show_layer(1);
    settle(&mut lighting, &mut chip, START_MS + 101, START_MS + 200);
    lighting.key_event(key, KeyEvent::Pressed, START_MS + 201);
    settle(&mut lighting, &mut chip, START_MS + 201, START_MS + 300);
    lighting.show_layer(0);
    settle(&mut lighting, &mut chip, START_MS + 301, START_MS + 400);
    assert_eq!(lighting.key_color(key).map(|color| color.green), Some(255));
}
