use std::{collections::HashMap, fmt::Write};

use openwoot::{
    keys::Key,
    lighting::{Canvas, EffectStack, Frame, KeyEvent, Painter, from_rgb565},
};
use openwoot_protocol::lighting::Effect;

use super::support::{
    COLUMNS, GEOMETRY, LENGTH, MAP, PAGES, ROWS, VARIANT, base_color, byte, effect,
};

const GOLDEN: &str = include_str!("effects.txt");

struct Scenario {
    name: String,
    step_ms: u32,
    frames: u32,
    effects: Vec<Effect>,
    events: Vec<(u32, Key, KeyEvent)>,
    travels: Vec<(u32, Key, u16)>,
}

struct Expected {
    digests: Vec<u64>,
    last_frame: String,
}

fn event(code: u32) -> KeyEvent {
    match code {
        0 => KeyEvent::Pressed,
        1 => KeyEvent::Released,
        2 => KeyEvent::Actuated,
        _ => KeyEvent::Reset,
    }
}

fn key(row: u32, column: u32) -> Key {
    Key::new(byte(row), byte(column))
}

fn parse_scenario(line: &str) -> Scenario {
    let mut words = line.split_whitespace().skip(1);
    let name = words.next().unwrap().to_owned();
    let step_ms = words.next().unwrap().parse().unwrap();
    let frames = words.next().unwrap().parse().unwrap();
    let mut scenario = Scenario {
        name,
        step_ms,
        frames,
        effects: Vec::new(),
        events: Vec::new(),
        travels: Vec::new(),
    };
    while let Some(tag) = words.next() {
        let mut number = || -> u32 { words.next().unwrap().parse().unwrap() };
        match tag {
            "L" => {
                let kind = number();
                scenario
                    .effects
                    .push(effect(kind, [number(), number(), number(), number()]));
            }
            "E" => {
                let frame = number();
                let key = key(number(), number());
                scenario.events.push((frame, key, event(number())));
            }
            "T" => {
                let frame = number();
                let key = key(number(), number());
                let travel = u16::try_from(number()).unwrap();
                scenario.travels.push((frame, key, travel));
            }
            other => panic!("unknown tag `{other}` in the golden data"),
        }
    }
    scenario
}

fn golden() -> (Vec<Scenario>, HashMap<String, Expected>) {
    let mut scenarios = Vec::new();
    let mut expected = HashMap::new();
    for line in GOLDEN.lines() {
        if line.starts_with("s ") {
            scenarios.push(parse_scenario(line));
        } else if let Some(results) = line.strip_prefix("g ") {
            let mut words: Vec<&str> = results.split_whitespace().collect();
            let name = words.remove(0).to_owned();
            let last_frame = words.pop().unwrap().to_owned();
            let digests = words
                .iter()
                .map(|digest| u64::from_str_radix(digest, 16).unwrap())
                .collect();
            expected.insert(
                name,
                Expected {
                    digests,
                    last_frame,
                },
            );
        }
    }
    (scenarios, expected)
}

fn key_colors(frame: &mut Frame<PAGES, LENGTH>) -> Vec<u8> {
    let canvas = Painter::new(frame, &MAP, &VARIANT, None);
    let mut colors = Vec::with_capacity(ROWS * COLUMNS * 3);
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            let color = canvas.key(row, column).unwrap_or_default();
            colors.extend([color.red, color.green, color.blue]);
        }
    }
    colors
}

fn digest(colors: &[u8]) -> u64 {
    colors.iter().fold(0xcbf2_9ce4_8422_2325, |hash, &byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn run(scenario: &Scenario) -> (Vec<u64>, String) {
    let mut effects = EffectStack::<ROWS, COLUMNS>::new();
    effects.apply(&scenario.effects);
    let mut travel = [[0_u16; COLUMNS]; ROWS];
    let mut digests = Vec::new();
    let mut colors = Vec::new();
    for frame_number in 1..=scenario.frames {
        let now_ms = 1000 + scenario.step_ms * frame_number;
        for &(_, key, event) in scenario
            .events
            .iter()
            .filter(|(frame, ..)| *frame == frame_number)
        {
            effects.event(key, event, now_ms, &VARIANT);
        }
        for &(_, key, value) in scenario
            .travels
            .iter()
            .filter(|(frame, ..)| *frame == frame_number)
        {
            travel[key.row()][key.column()] = value;
        }
        if !scenario.travels.is_empty() {
            for (row, values) in travel.iter().enumerate() {
                for (column, &value) in values.iter().enumerate() {
                    effects.feed(Key::new(row as u8, column as u8), value, &());
                }
            }
        }
        let mut frame = Frame::new();
        {
            let mut canvas = Painter::new(&mut frame, &MAP, &VARIANT, None);
            for row in 0..ROWS {
                for column in 0..COLUMNS {
                    canvas.set_key(row, column, from_rgb565(base_color(row, column)));
                }
            }
            effects.render(&mut canvas, GEOMETRY, scenario.step_ms);
        }
        colors = key_colors(&mut frame);
        digests.push(digest(&colors));
    }
    let hex = colors.iter().fold(String::new(), |mut hex, byte| {
        write!(hex, "{byte:02x}").unwrap();
        hex
    });
    (digests, hex)
}

#[test]
fn every_effect_matches_the_frames_of_the_reference_firmware() {
    let (scenarios, expected) = golden();
    assert!(scenarios.len() >= 30);
    let mut failures = Vec::new();
    for scenario in &scenarios {
        let expected = &expected[&scenario.name];
        let (digests, last_frame) = run(scenario);
        if let Some(frame) = digests
            .iter()
            .zip(&expected.digests)
            .position(|(actual, wanted)| actual != wanted)
        {
            failures.push(format!(
                "{} differs from frame {}",
                scenario.name,
                frame + 1
            ));
        } else if last_frame != expected.last_frame {
            failures.push(format!("{} differs in its last frame", scenario.name));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn scenarios_cover_every_effect_kind() {
    let (scenarios, _) = golden();
    let mut kinds: Vec<_> = scenarios
        .iter()
        .flat_map(|scenario| &scenario.effects)
        .map(std::mem::discriminant)
        .collect();
    kinds.sort_by_key(|kind| format!("{kind:?}"));
    kinds.dedup();
    assert_eq!(kinds.len(), 13);
}
