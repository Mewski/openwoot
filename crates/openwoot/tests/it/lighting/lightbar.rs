use std::fmt::Write;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use openwoot::{
    keys::Key,
    lighting::{
        Bar, Canvas, Frame, FramePage, Gains, KeyEvent, Led, LedMap, LedPosition, Lightbar,
        Lighting, LockState, Painter, ProfileState, Signals, lightbar_profile,
    },
    profile,
};
use openwoot_protocol::{
    heapless::Vec as Bounded,
    keys::Position,
    lightbar::{
        Background, Blink, Builtin, Comparator, Comparison, Condition, Custom, Effect, Entries,
        Entry, Group, HostSignal, KeyRect, Logic, Meter, Mirror, Operand, ProfileIndicator, Range,
        Settings, Signal, Solid, Target, Term, Waveform,
    },
    lighting::{Color, Rgb},
};

use super::{COLUMNS, GEOMETRY, LENGTH, ROWS, VARIANT};

const GOLDEN: &str = include_str!("lightbar.txt");
const BAR_PAGES: usize = 10;
const KEY_LEDS: usize = ROWS * COLUMNS;
const BAR_LEDS: [Led; 10] = [
    Led(103),
    Led(104),
    Led(105),
    Led(106),
    Led(107),
    Led(108),
    Led(109),
    Led(110),
    Led(111),
    Led(112),
];

static POSITIONS: [Option<LedPosition>; KEY_LEDS + 11] = positions();
static BAR_MAP: LedMap<BAR_PAGES> = LedMap {
    positions: &POSITIONS,
    pages: pages(),
    gains: Gains::FULL,
};

const fn positions() -> [Option<LedPosition>; KEY_LEDS + 11] {
    let per_page = LENGTH / 3;
    let mut positions = [None; KEY_LEDS + 11];
    let mut led = 1;
    while led < KEY_LEDS + 11 {
        let index = led - 1;
        positions[led] = Some(LedPosition::new(
            (index / per_page) as u8,
            (index % per_page * 3) as u8,
        ));
        led += 1;
    }
    positions
}

const fn pages() -> [FramePage; BAR_PAGES] {
    let mut pages = [FramePage::new(0, 0, LENGTH as u8); BAR_PAGES];
    let mut page = 0;
    while page < BAR_PAGES {
        let first = page * LENGTH;
        pages[page] = FramePage::new((first / 180) as u8, (first % 180) as u8, LENGTH as u8);
        page += 1;
    }
    pages
}

struct Tokens<'a>(std::str::SplitWhitespace<'a>);

impl<'a> Tokens<'a> {
    fn next(&mut self) -> &'a str {
        self.0.next().unwrap()
    }

    fn number<T: std::str::FromStr>(&mut self) -> T
    where
        T::Err: std::fmt::Debug,
    {
        self.next().parse().unwrap()
    }

    fn rgb(&mut self) -> Rgb {
        rgb_of(self.next())
    }

    fn color(&mut self) -> Color {
        let Rgb { red, green, blue } = self.rgb();
        Color {
            red,
            green,
            blue,
            alpha: 255,
        }
    }

    fn optional_color(&mut self) -> Option<Color> {
        match self.next() {
            "-" => None,
            hex => Some(color_of(rgb_of(hex))),
        }
    }

    fn range(&mut self) -> Range {
        Range {
            first: self.number(),
            last: self.number(),
        }
    }

    fn rect(&mut self) -> KeyRect {
        KeyRect {
            first: Position {
                row: self.number(),
                column: self.number(),
            },
            last: Position {
                row: self.number(),
                column: self.number(),
            },
        }
    }

    fn target(&mut self) -> Target {
        match self.next() {
            "L" => Target::Lightbar(self.range()),
            _ => Target::Keys(self.rect()),
        }
    }

    fn signal(&mut self) -> Signal {
        signal_of(self.next())
    }

    fn entry(&mut self) -> Entry {
        let effect = match self.next() {
            "M" => Effect::Meter(Meter {
                signal: self.signal(),
                maximum: self.number(),
                color: self.color(),
                background: self.optional_color(),
                gradient_start: self.optional_color(),
                range: self.range(),
                reverse: self.number::<u8>() != 0,
            }),
            "I" => {
                let range = self.range();
                let mode: u8 = self.number();
                let fixed = self.optional_color();
                let background = match mode {
                    0 => Background::ProfileColor,
                    1 => Background::Transparent,
                    2 => Background::Fixed(fixed.unwrap()),
                    3 => Background::Contrast,
                    _ => Background::DimmedProfileColor,
                };
                Effect::ProfileIndicator(ProfileIndicator {
                    range,
                    background,
                    segments: self.number(),
                })
            }
            "R" => Effect::Mirror(Mirror {
                range: self.range(),
                source: self.rect(),
            }),
            "K" => Effect::Blink(Blink {
                from: self.color(),
                to: self.color(),
                period_ms: self.number(),
                waveform: if self.number::<u8>() == 0 {
                    Waveform::Square
                } else {
                    Waveform::Triangle
                },
                target: self.target(),
            }),
            "S" => Effect::Solid(Solid {
                color: self.color(),
                target: self.target(),
            }),
            _ => {
                let range = self.range();
                let count: usize = self.number();
                Effect::Custom(Custom {
                    colors: (0..count).map(|_| self.rgb()).collect(),
                    range,
                })
            }
        };
        let condition = match self.next() {
            "C" => {
                let logic = logic_of(self.number());
                let groups: usize = self.number();
                Some(Condition {
                    logic,
                    groups: (0..groups).map(|_| self.group()).collect(),
                })
            }
            _ => None,
        };
        Entry { condition, effect }
    }

    fn group(&mut self) -> Group {
        let logic = logic_of(self.number());
        let terms: usize = self.number();
        Group {
            logic,
            terms: (0..terms).map(|_| self.term()).collect(),
        }
    }

    fn term(&mut self) -> Term {
        if self.next() == "T" {
            return Term::Trigger {
                signal: self.signal(),
                duration_ms: self.number(),
            };
        }
        let signal = self.signal();
        let comparator = match self.number::<u8>() {
            0 => Comparator::Equal,
            1 => Comparator::NotEqual,
            2 => Comparator::Greater,
            3 => Comparator::GreaterOrEqual,
            4 => Comparator::Less,
            _ => Comparator::LessOrEqual,
        };
        let operand = match self.next() {
            constant if constant.starts_with('k') => {
                Operand::Constant(constant[1..].parse().unwrap())
            }
            other => Operand::Signal(signal_of(other)),
        };
        let hold_ms = match self.next() {
            "-" => None,
            hold => Some(hold.parse().unwrap()),
        };
        Term::Compare(Comparison {
            signal,
            comparator,
            operand,
            hold_ms,
        })
    }
}

fn rgb_of(hex: &str) -> Rgb {
    let value = u32::from_str_radix(hex, 16).unwrap();
    let [_, red, green, blue] = value.to_be_bytes();
    Rgb { red, green, blue }
}

const fn color_of(rgb: Rgb) -> Color {
    Color {
        red: rgb.red,
        green: rgb.green,
        blue: rgb.blue,
        alpha: 255,
    }
}

fn signal_of(token: &str) -> Signal {
    let id: u32 = token[1..].parse().unwrap();
    if token.starts_with('B') {
        Signal::Builtin(builtin_of(id))
    } else {
        Signal::Host(id)
    }
}

fn builtin_of(id: u32) -> Builtin {
    [
        Builtin::ActiveProfile,
        Builtin::AnalogLevel,
        Builtin::KeyPressed,
        Builtin::CapsLock,
        Builtin::ScrollLock,
        Builtin::NumLock,
        Builtin::ProfileCount,
        Builtin::ActiveLayer,
        Builtin::Brightness,
        Builtin::KeyRate,
        Builtin::ProfileColor,
    ][id as usize]
}

fn logic_of(code: u8) -> Logic {
    match code {
        0 => Logic::Single,
        1 => Logic::All,
        _ => Logic::Any,
    }
}

fn run(line: &str) -> (String, String) {
    let mut tokens = Tokens(line.split_whitespace());
    assert_eq!(tokens.next(), "s");
    let name = tokens.next().to_owned();
    let mut settings = Settings {
        enabled: tokens.number::<u8>() != 0,
        entries: Bounded::new(),
        use_global: tokens.number::<u8>() != 0,
        brightness: u8::try_from(tokens.number::<i32>()).ok(),
    };
    let key_brightness: u8 = tokens.number();
    let mut global = Entries::new();
    let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
    let mut frame = Frame::<BAR_PAGES, LENGTH>::new();
    let mut canvas = Painter::new(&mut frame, &BAR_MAP, &VARIANT, None);
    let mut output = name.clone();
    while let Some(token) = tokens.0.next() {
        match token {
            "G" => global.push(tokens.entry()).unwrap(),
            "P" => settings.entries.push(tokens.entry()).unwrap(),
            "x" => bar.configure(&settings, &global, key_brightness),
            "b" => {
                let builtin = builtin_of(tokens.number());
                bar.publish(Signal::Builtin(builtin), tokens.number());
            }
            "c" => {
                let bits = 0xFF00_0000 | u32::from_str_radix(tokens.next(), 16).unwrap();
                bar.publish(Signal::Builtin(Builtin::ProfileColor), f32::from_bits(bits));
            }
            "h" => {
                let id = tokens.number();
                let value = tokens.number();
                bar.set_host_signals(&[HostSignal { id, value }]);
            }
            "a" => bar.activate_host_signals(),
            "l" => {
                let ids: Vec<String> = bar.host_signal_ids().iter().map(u32::to_string).collect();
                write!(output, " ids:{}", ids.join(",")).unwrap();
            }
            "k" => {
                let row: usize = tokens.number();
                let column: usize = tokens.number();
                canvas.set_key(row, column, tokens.rgb());
            }
            "f" => bar.set_fast_scan(tokens.number::<u8>() != 0),
            "m" => bar.set_host_control(tokens.number::<u8>() != 0),
            "e" => bar.apply_brightness_change(tokens.number()),
            "r" => {
                let elapsed: u16 = tokens.number();
                let needs = bar.needs_render(elapsed);
                if needs {
                    bar.render(&mut canvas, elapsed);
                }
                write!(output, " {}/", u8::from(needs)).unwrap();
                for led in BAR_LEDS {
                    let color = canvas.led(led).unwrap();
                    write!(
                        output,
                        "{:02x}{:02x}{:02x}",
                        color.red, color.green, color.blue
                    )
                    .unwrap();
                }
                write!(output, "/{:016x}/{}", key_hash(&canvas), bar.brightness()).unwrap();
            }
            other => panic!("unknown step `{other}` in `{name}`"),
        }
    }
    (name, output)
}

fn key_hash(canvas: &impl Canvas) -> u64 {
    (0..ROWS)
        .flat_map(|row| (0..COLUMNS).map(move |column| (row, column)))
        .map(|(row, column)| canvas.key(row, column).unwrap_or_default())
        .flat_map(|color| [color.red, color.green, color.blue])
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
}

#[test]
fn every_light_bar_scenario_matches_the_reference_firmware() {
    let lines: Vec<&str> = GOLDEN.lines().collect();
    assert!(lines.len() >= 60);
    for pair in lines.chunks(2) {
        let (name, output) = run(pair[0]);
        assert_eq!(output, pair[1], "scenario `{name}`");
    }
}

#[test]
fn page_writes_of_the_light_bar_use_its_own_brightness() {
    let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
    let settings = Settings {
        enabled: true,
        entries: Bounded::new(),
        use_global: true,
        brightness: Some(128),
    };
    bar.configure(&settings, &Entries::new(), 255);
    assert!(bar.take_brightness_change());
    assert!(!bar.take_brightness_change());
    let page = usize::from(BAR_MAP.position(BAR_LEDS[0]).unwrap().page);
    let offset = usize::from(BAR_MAP.position(BAR_LEDS[0]).unwrap().offset);
    let linear = [200; LENGTH];
    let mut output = [7; LENGTH];
    bar.apply_page_brightness(page, &linear, &mut output, 255);
    let expected =
        openwoot::lighting::output_value(200, openwoot::lighting::output_scale(128, 255));
    assert_eq!(&output[offset..offset + 3], &[expected; 3]);
    assert!(output[..offset].iter().all(|value| *value == 7));
    let mut untouched = [7; LENGTH];
    bar.apply_page_brightness(0, &linear, &mut untouched, 255);
    assert_eq!(untouched, [7; LENGTH]);
}

fn meter(signal: Signal, maximum: f32, first: u8, last: u8) -> Entry {
    Entry {
        condition: None,
        effect: Effect::Meter(Meter {
            signal,
            maximum,
            color: color_of(rgb_of("ffffff")),
            background: Some(color_of(rgb_of("000000"))),
            gradient_start: None,
            range: Range { first, last },
            reverse: false,
        }),
    }
}

fn bar_colors(bar: &mut Bar) -> Vec<String> {
    let mut frame = Frame::<BAR_PAGES, LENGTH>::new();
    let mut canvas = Painter::new(&mut frame, &BAR_MAP, &VARIANT, None);
    bar.request_redraw();
    bar.render(&mut canvas, 16);
    BAR_LEDS
        .iter()
        .map(|led| {
            let color = canvas.led(*led).unwrap();
            format!("{:02x}{:02x}{:02x}", color.red, color.green, color.blue)
        })
        .collect()
}

#[test]
fn host_signals_travel_from_the_configuration_to_the_light_bar_and_back() {
    let shared = profile::Shared::<CriticalSectionRawMutex>::new();
    let mut lighting = Lighting::<_, ROWS, COLUMNS, BAR_PAGES, LENGTH>::new(
        &BAR_MAP,
        &VARIANT,
        GEOMETRY,
        Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS),
        0,
    );
    let mut settings = lightbar_profile(Some(90));
    settings
        .entries
        .push(meter(Signal::Host(42), 1.0, 0, 3))
        .unwrap();
    let mut global = Entries::new();
    global.push(meter(Signal::Host(7), 2.0, 4, 9)).unwrap();
    shared.edit(|appearance| appearance.lightbar = settings);
    shared.set_global_lightbar(&global);
    shared.activate(false);
    shared.apply(&mut lighting, 0);
    assert_eq!(shared.host_signals().ids().as_slice(), &[] as &[u32]);
    shared.apply(&mut lighting, 4);
    assert_eq!(shared.host_signals().ids().as_slice(), &[7, 42]);
    assert_eq!(lighting.hooks().brightness(), 90);

    shared.host_signals().set(&[
        HostSignal { id: 42, value: 0.5 },
        HostSignal { id: 7, value: 2.0 },
        HostSignal { id: 99, value: 1.0 },
    ]);
    shared.apply(&mut lighting, 8);
    assert_eq!(
        bar_colors(lighting.hooks_mut()),
        [
            "ffffff", "ffffff", "000000", "000000", "ffffff", "ffffff", "ffffff", "ffffff",
            "ffffff", "ffffff"
        ]
    );

    shared.set_profile_state(ProfileState {
        slot: 1,
        count: 3,
        color: rgb_of("ff2828"),
    });
    let mut indicator = lightbar_profile(None);
    indicator.use_global = false;
    indicator
        .entries
        .push(Entry {
            condition: None,
            effect: Effect::ProfileIndicator(ProfileIndicator {
                range: Range { first: 0, last: 9 },
                background: Background::Transparent,
                segments: 1,
            }),
        })
        .unwrap();
    shared.change(profile::Changes::LIGHTBAR, |appearance| {
        appearance.lightbar = indicator;
    });
    shared.apply(&mut lighting, 12);
    assert_eq!(
        bar_colors(lighting.hooks_mut())[..6],
        ["000000", "000000", "000000", "ff2828", "ff2828", "ff2828"]
    );
    shared.apply(&mut lighting, 16);
    assert_eq!(shared.host_signals().ids().as_slice(), &[] as &[u32]);
}

#[test]
fn built_in_signals_follow_keys_locks_layers_and_the_key_rate() {
    let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
    let mut settings = lightbar_profile(None);
    for (position, (builtin, maximum)) in (0..).zip([
        (Builtin::KeyPressed, 1.0),
        (Builtin::CapsLock, 1.0),
        (Builtin::ActiveLayer, 3.0),
        (Builtin::KeyRate, 2.0),
    ]) {
        settings
            .entries
            .push(meter(Signal::Builtin(builtin), maximum, position, position))
            .unwrap();
    }
    bar.configure(&settings, &Entries::new(), 255);
    assert_eq!(bar_colors(&mut bar)[..4], ["000000"; 4]);

    let key = Key::new(2, 3);
    bar.key_event(key, KeyEvent::Pressed);
    bar.key_event(key, KeyEvent::Actuated);
    bar.locks_changed(LockState {
        caps_lock: true,
        ..LockState::default()
    });
    bar.layer_changed(3);
    bar.tick(15);
    assert_eq!(
        bar_colors(&mut bar)[..4],
        ["ffffff", "ffffff", "ffffff", "7e7e7e"]
    );

    bar.key_event(key, KeyEvent::Released);
    bar.travel_changed(4095);
    let mut now = 15;
    for _ in 0..66 {
        now += 15;
        bar.tick(now);
    }
    assert_eq!(
        bar_colors(&mut bar)[..4],
        ["000000", "ffffff", "ffffff", "000000"]
    );
}
