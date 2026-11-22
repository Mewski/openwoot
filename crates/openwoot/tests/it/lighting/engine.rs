use openwoot::{
    keys::Key,
    lighting::{Indicator, KeyEvent, LockState, Overlay, OverlayId, Span, rgb},
};
use openwoot_protocol::lighting::{Effects, Rgb};

use super::support::{Chip, PAGES, START_MS, TestLighting, effect, lighting, settle};

const GOLDEN: &str = include_str!("outputs.txt");

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
