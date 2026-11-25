use std::{collections::HashMap, fmt::Write};

use openwoot::{
    keys::Key,
    lighting::{
        Canvas, EffectStack, Frame, FramePage, Gains, Geometry, KeyEvent, Led, LedMap, LedPosition,
        Painter, TRAVEL_FULL, Variant, from_rgb565, rgb,
    },
};
use openwoot_protocol::lighting::{Effect, Rgb};

use super::support::{
    COLUMNS, GEOMETRY, LENGTH, MAP, PAGES, ROWS, VARIANT, base_color, byte, effect,
};

const GOLDEN: &str = include_str!("../../fixtures/lighting/effects.txt");
const SCENARIOS: usize = 37;

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
        3 => KeyEvent::Reset,
        other => panic!("no key event {other} in the golden data"),
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
    let frames = 1..=scenario.frames;
    assert!(
        scenario
            .events
            .iter()
            .map(|(frame, ..)| frame)
            .chain(scenario.travels.iter().map(|(frame, ..)| frame))
            .all(|frame| frames.contains(frame)),
        "{} has an event outside its {} frames",
        scenario.name,
        scenario.frames
    );
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
            let duplicate = expected.insert(
                name.clone(),
                Expected {
                    digests,
                    last_frame,
                },
            );
            assert!(duplicate.is_none(), "`{name}` has two result lines");
        } else {
            panic!("unknown line `{line}` in the golden data");
        }
    }
    assert_eq!(scenarios.len(), SCENARIOS);
    assert_eq!(expected.len(), SCENARIOS);
    for scenario in &scenarios {
        assert!(
            expected.contains_key(&scenario.name),
            "`{}` has no result line",
            scenario.name
        );
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
    let mut failures = Vec::new();
    for scenario in &scenarios {
        let expected = &expected[&scenario.name];
        let (digests, last_frame) = run(scenario);
        if digests.len() != expected.digests.len() {
            failures.push(format!(
                "{} renders {} frames but has {} digests",
                scenario.name,
                digests.len(),
                expected.digests.len()
            ));
        } else if let Some(frame) = digests
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

fn rendered(effects: &mut EffectStack<ROWS, COLUMNS>) -> Vec<u8> {
    let mut frame = Frame::new();
    {
        let mut canvas = Painter::new(&mut frame, &MAP, &VARIANT, None);
        for row in 0..ROWS {
            for column in 0..COLUMNS {
                canvas.set_key(row, column, from_rgb565(base_color(row, column)));
            }
        }
        effects.render(&mut canvas, GEOMETRY, 16);
    }
    key_colors(&mut frame)
}

#[test]
fn releasing_the_keys_returns_the_analog_effects_to_their_released_frame() {
    for kind in [7, 8, 9, 10] {
        let first = if kind == 10 { 1 } else { 40 };
        let layers = [effect(kind, [first, 0, 0, 0])];
        let mut untouched = EffectStack::<ROWS, COLUMNS>::new();
        untouched.apply(&layers);
        let released = rendered(&mut untouched);
        let mut effects = EffectStack::<ROWS, COLUMNS>::new();
        effects.apply(&layers);
        for column in COLUMNS - 3..COLUMNS {
            effects.feed(Key::new(5, column as u8), TRAVEL_FULL, &());
        }
        assert_ne!(rendered(&mut effects), released, "kind {kind}");
        effects.release_keys();
        assert!(effects.needs_render(), "kind {kind}");
        assert_eq!(rendered(&mut effects), released, "kind {kind}");
    }
}

fn lit_columns(effects: &mut EffectStack<ROWS, COLUMNS>) -> usize {
    let mut frame = Frame::<PAGES, LENGTH>::new();
    let mut canvas = Painter::new(&mut frame, &MAP, &VARIANT, None);
    for row in 0..ROWS {
        for column in 0..COLUMNS {
            canvas.set_key(row, column, rgb(200, 200, 200));
        }
    }
    effects.render(&mut canvas, GEOMETRY, 16);
    (0..COLUMNS)
        .filter(|&column| canvas.key(1, column) == Some(rgb(200, 200, 200)))
        .count()
}

#[test]
fn a_scanner_wider_than_the_board_always_lights_a_window_however_it_was_set() {
    for width in [17, 30, 127, 128, 255] {
        let mut loaded = EffectStack::<ROWS, COLUMNS>::new();
        loaded.apply(&[effect(2, [width, 20, 0, 1])]);
        let mut edited = EffectStack::<ROWS, COLUMNS>::new();
        edited.apply(&[effect(2, [4, 20, 0, 1])]);
        edited.apply(&[effect(2, [width, 20, 0, 1])]);
        for frame in 0..1500 {
            let lit = lit_columns(&mut loaded);
            assert!(lit > 0, "width {width} lights nothing in frame {frame}");
            assert_eq!(
                lit_columns(&mut edited),
                lit,
                "width {width}, frame {frame}"
            );
        }
    }
}

static GAINED_POSITIONS: [Option<LedPosition>; 2] = [None, Some(LedPosition::new(0, 0))];

static GAINED_MAP: LedMap<1> = LedMap {
    positions: &GAINED_POSITIONS,
    pages: [FramePage::new(0, 0, 3)],
    gains: Gains {
        red: 232,
        green: 255,
        blue: 255,
    },
};

static FULL_MAP: LedMap<1> = LedMap {
    positions: &GAINED_POSITIONS,
    pages: [FramePage::new(0, 0, 3)],
    gains: Gains::FULL,
};

static ONE_KEY: Variant<1, 1> = Variant {
    leds: [[Some(Led(1))]],
    spacebar: Key::new(0, 0),
    spacebar_fill: &[],
};

fn fixed_fade_reds(map: &'static LedMap<1>) -> Vec<u8> {
    let mut effects = EffectStack::<1, 1>::new();
    effects.apply(&[effect(6, [255, 4, 2, 0])]);
    effects.event(Key::new(0, 0), KeyEvent::Pressed, 0, &ONE_KEY);
    let geometry = Geometry {
        first_key_row: 0,
        level_meter_segments: 0,
    };
    (0..80)
        .map(|_| {
            let mut frame = Frame::<1, 3>::new();
            let mut canvas = Painter::new(&mut frame, map, &ONE_KEY, None);
            canvas.set_key(0, 0, rgb(255, 255, 255));
            effects.render(&mut canvas, geometry, 16);
            canvas.key(0, 0).unwrap().red
        })
        .collect()
}

#[test]
fn the_fixed_colour_crossfade_applies_the_board_gains_once() {
    let full = fixed_fade_reds(&FULL_MAP);
    let gained = fixed_fade_reds(&GAINED_MAP);
    assert!(full.iter().any(|&red| red != 0 && red != 255));
    for (frame, (&full, &gained)) in full.iter().zip(&gained).enumerate() {
        let once = u8::try_from((232 * (u32::from(full) + 1)) >> 8).unwrap();
        assert!(
            gained.abs_diff(once) <= 1,
            "frame {frame}: {gained} with the gains against {once} for one gain"
        );
    }
}

#[test]
fn applying_unchanged_settings_keeps_every_effect_running() {
    for layer in [
        effect(1, [0, 8, 1, 0]),
        effect(2, [4, 20, 40, 0]),
        effect(3, [15, 30, 0, 0]),
        effect(4, [0, 8, 0, 0]),
        effect(5, [1, 20, 30, 40]),
        effect(6, [40, 4, 0, 0]),
        effect(11, [40, 0, 0, 0]),
        effect(14, [63488, 31, 40, 30]),
        effect(15, [63488, 2016, 30, 50]),
    ] {
        let layers = [layer];
        let mut plain = EffectStack::<ROWS, COLUMNS>::new();
        plain.apply(&layers);
        let mut reapplied = EffectStack::<ROWS, COLUMNS>::new();
        reapplied.apply(&layers);
        for effects in [&mut plain, &mut reapplied] {
            effects.event(Key::new(2, 3), KeyEvent::Pressed, 1000, &VARIANT);
        }
        for frame in 0..20 {
            let expected = rendered(&mut plain);
            if frame % 5 == 4 {
                reapplied.apply(&layers);
            }
            assert_eq!(
                rendered(&mut reapplied),
                expected,
                "{layer:?} frame {frame}"
            );
        }
    }
}

const RED_565: u32 = 0xF800;

struct BlueKeyUnderRedFade {
    rest_then_pressed: Vec<Rgb>,
    animating_after_last: bool,
}

fn blue_key_under_red_fade(
    base_level: u32,
    flags: u32,
    frames_after_press: usize,
) -> BlueKeyUnderRedFade {
    let mut effects = EffectStack::<1, 1>::new();
    effects.apply(&[effect(6, [base_level, 4, flags, RED_565])]);
    let geometry = Geometry {
        first_key_row: 0,
        level_meter_segments: 0,
    };
    let render = |effects: &mut EffectStack<1, 1>| {
        let mut frame = Frame::<1, 3>::new();
        let mut canvas = Painter::new(&mut frame, &FULL_MAP, &ONE_KEY, None);
        canvas.set_key(0, 0, rgb(0, 0, 255));
        effects.render(&mut canvas, geometry, 16);
        canvas.key(0, 0).unwrap()
    };
    let mut colors = vec![render(&mut effects)];
    effects.event(Key::new(0, 0), KeyEvent::Pressed, 0, &ONE_KEY);
    colors.extend((0..frames_after_press).map(|_| render(&mut effects)));
    BlueKeyUnderRedFade {
        rest_then_pressed: colors,
        animating_after_last: effects.needs_render(),
    }
}

fn light(color: Rgb) -> u32 {
    u32::from(color.red) + u32::from(color.green) + u32::from(color.blue)
}

const FIXED: u32 = 2;
const INVERTED: u32 = 4;

#[test]
fn an_inverted_fixed_colour_fade_rests_on_its_colour_and_a_press_takes_the_light_away() {
    let BlueKeyUnderRedFade {
        rest_then_pressed: colors,
        animating_after_last: animating,
    } = blue_key_under_red_fade(0, FIXED | INVERTED, 80);
    let rest = colors[0];
    assert!(
        rest.red > 250 && rest.green == 0 && rest.blue == 0,
        "{rest:?}"
    );
    assert_eq!(colors[1], rgb(0, 0, 0));
    for (frame, pair) in colors[1..].windows(2).enumerate() {
        assert!(
            light(pair[1]) >= light(pair[0]),
            "frame {}: {:?} after {:?}",
            frame + 2,
            pair[1],
            pair[0]
        );
    }
    assert!(
        colors[2..]
            .iter()
            .any(|&color| color != rgb(0, 0, 0) && color != rest)
    );
    assert_eq!(colors.last(), Some(&rest));
    assert!(!animating);
}

#[test]
fn an_inverted_fixed_colour_fade_mirrors_the_plain_one() {
    let BlueKeyUnderRedFade {
        rest_then_pressed: plain,
        animating_after_last: plain_animating,
    } = blue_key_under_red_fade(80, FIXED, 80);
    let BlueKeyUnderRedFade {
        rest_then_pressed: inverted,
        animating_after_last: inverted_animating,
    } = blue_key_under_red_fade(80, FIXED | INVERTED, 80);
    assert_eq!(
        inverted[0], plain[1],
        "an inverted key rests where a press starts"
    );
    assert_eq!(
        inverted[1], plain[0],
        "an inverted press starts where a key rests"
    );
    assert!(
        inverted[1].red == 0 && inverted[1].blue > 0,
        "{:?}",
        inverted[1]
    );
    assert!(
        inverted
            .iter()
            .any(|color| color.red != 0 && color.blue != 0),
        "an inverted press crossfades from the key colour to the fixed colour"
    );
    assert_eq!(plain.last(), Some(&plain[0]));
    assert_eq!(inverted.last(), Some(&inverted[0]));
    assert!(!plain_animating && !inverted_animating);
}

#[test]
fn an_inverted_press_dims_to_the_same_base_with_a_fixed_colour_and_with_the_key_colour() {
    for base_level in [0, 80, 200] {
        let key_color = blue_key_under_red_fade(base_level, INVERTED, 1).rest_then_pressed;
        let fixed = blue_key_under_red_fade(base_level, FIXED | INVERTED, 1).rest_then_pressed;
        assert_eq!(fixed[1], key_color[1], "base level {base_level}");
    }
}
