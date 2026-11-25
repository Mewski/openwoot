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
        Settings, Signal, Solid, Target, Term, Title, Waveform,
    },
    lighting::{Color, Rgb},
};

use super::support::{COLUMNS, GEOMETRY, LENGTH, ROWS, VARIANT};

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
        Entry {
            title: Title::new(),
            condition,
            effect,
        }
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
        title: Title::new(),
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
            title: Title::new(),
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

static GAINED_MAP: LedMap<BAR_PAGES> = LedMap {
    positions: &POSITIONS,
    pages: pages(),
    gains: Gains {
        red: 232,
        green: 255,
        blue: 255,
    },
};

fn solid_on(first: u8, last: u8, condition: Option<Condition>) -> Entry {
    Entry {
        title: Title::new(),
        condition,
        effect: Effect::Solid(Solid {
            color: color_of(rgb_of("ff0000")),
            target: Target::Lightbar(Range { first, last }),
        }),
    }
}

const fn compare(
    signal: Signal,
    comparator: Comparator,
    operand: Operand,
    hold_ms: Option<u32>,
) -> Term {
    Term::Compare(Comparison {
        signal,
        comparator,
        operand,
        hold_ms,
    })
}

fn condition_of(logic: Logic, group_logic: Logic, terms: &[Term]) -> Condition {
    Condition {
        logic,
        groups: Bounded::from_iter([Group {
            logic: group_logic,
            terms: terms.iter().copied().collect(),
        }]),
    }
}

fn caps_is_on(logic: Logic, group_logic: Logic) -> Condition {
    condition_of(
        logic,
        group_logic,
        &[compare(
            Signal::Builtin(Builtin::CapsLock),
            Comparator::Equal,
            Operand::Constant(1.0),
            None,
        )],
    )
}

fn profile_of(entries: impl IntoIterator<Item = Entry>) -> Settings {
    let mut settings = lightbar_profile(None);
    settings.entries = entries.into_iter().collect();
    settings
}

fn with_engine(bar: Bar) -> Lighting<Bar, ROWS, COLUMNS, BAR_PAGES, LENGTH> {
    let mut lighting = Lighting::new(&BAR_MAP, &VARIANT, GEOMETRY, bar, 0);
    let white = color_of(rgb_of("ffffff"));
    lighting.apply_settings(
        &openwoot_protocol::lighting::Settings {
            brightness: 255,
            indicators: openwoot_protocol::lighting::Indicators {
                caps_lock: white,
                scroll_lock: white,
                num_lock: white,
                layer_lock: white,
            },
            effects: Bounded::new(),
        },
        0,
    );
    let base: openwoot_protocol::lighting::KeyColors =
        core::iter::repeat_n(rgb_of("0a141e"), ROWS * COLUMNS).collect();
    lighting.set_base_colors(&base);
    lighting
}

#[test]
fn a_profile_indicator_on_one_led_is_drawn_and_then_rests() {
    let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
    let settings = profile_of([Entry {
        title: Title::new(),
        condition: None,
        effect: Effect::ProfileIndicator(ProfileIndicator {
            range: Range { first: 0, last: 0 },
            background: Background::Transparent,
            segments: 1,
        }),
    }]);
    bar.configure(&settings, &Entries::new(), 255);
    bar.profile_changed(ProfileState {
        slot: 0,
        count: 1,
        color: rgb_of("ff2828"),
    });
    let mut frame = Frame::<BAR_PAGES, LENGTH>::new();
    let mut canvas = Painter::new(&mut frame, &BAR_MAP, &VARIANT, None);
    let mut asked = 0;
    for _ in 0..100 {
        if bar.needs_render(16) {
            asked += 1;
            bar.render(&mut canvas, 16);
        }
    }
    assert_eq!(asked, 1);
    assert_eq!(canvas.led(BAR_LEDS[0]), Some(rgb_of("ff2828")));
    assert_eq!(canvas.led(BAR_LEDS[1]), Some(rgb_of("000000")));
}

#[test]
fn keys_painted_by_an_entry_get_their_colours_back_when_its_condition_falls() {
    let mut lighting = with_engine(Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS));
    let mut settings = lightbar_profile(None);
    settings.use_global = false;
    settings
        .entries
        .push(Entry {
            title: Title::new(),
            condition: Some(caps_is_on(Logic::All, Logic::All)),
            effect: Effect::Solid(Solid {
                color: color_of(rgb_of("ff0000")),
                target: Target::Keys(KeyRect {
                    first: Position { row: 1, column: 1 },
                    last: Position { row: 1, column: 3 },
                }),
            }),
        })
        .unwrap();
    lighting
        .hooks_mut()
        .configure(&settings, &Entries::new(), 255);
    let key = Key::new(1, 2);
    for now in (0..200).step_by(4) {
        lighting.update(now, &());
    }
    assert_eq!(lighting.key_color(key), Some(rgb_of("0a141e")));
    lighting.hooks_mut().locks_changed(LockState {
        caps_lock: true,
        ..LockState::default()
    });
    for now in (200..400).step_by(4) {
        lighting.update(now, &());
    }
    assert_eq!(lighting.key_color(key), Some(rgb_of("ff0000")));
    lighting.hooks_mut().locks_changed(LockState::default());
    for now in (400..600).step_by(4) {
        lighting.update(now, &());
    }
    assert_eq!(lighting.key_color(key), Some(rgb_of("0a141e")));
    assert!(!lighting.hooks().has_stale_keys());
}

#[test]
fn the_brightness_signal_follows_the_key_brightness_from_the_start() {
    let mut lighting = with_engine(Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS));
    let settings = profile_of([meter(Signal::Builtin(Builtin::Brightness), 100.0, 0, 9)]);
    let key_brightness = lighting.brightness();
    lighting
        .hooks_mut()
        .configure(&settings, &Entries::new(), key_brightness);
    assert_eq!(bar_colors(lighting.hooks_mut()), ["ffffff"; 10]);
    lighting.step_brightness(false, 200);
    let stepped = bar_colors(lighting.hooks_mut());
    assert_eq!(stepped[..9], ["ffffff"; 9]);
    assert_ne!(stepped[9], "ffffff");
}

#[test]
fn host_values_reach_meters_bound_later_and_survive_binding_again() {
    let shared = profile::Shared::<CriticalSectionRawMutex>::new();
    let mut lighting = Lighting::<_, ROWS, COLUMNS, BAR_PAGES, LENGTH>::new(
        &BAR_MAP,
        &VARIANT,
        GEOMETRY,
        Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS),
        0,
    );
    let mut settings = lightbar_profile(None);
    settings
        .entries
        .push(meter(Signal::Host(42), 1.0, 0, 3))
        .unwrap();
    shared.edit(|appearance| appearance.lightbar = settings);
    shared.activate(false);
    shared
        .host_signals()
        .set(&[HostSignal { id: 42, value: 1.0 }]);
    shared.apply(&mut lighting, 0);
    assert_eq!(bar_colors(lighting.hooks_mut())[..4], ["ffffff"; 4]);

    shared.activate(false);
    shared.apply(&mut lighting, 4);
    assert_eq!(bar_colors(lighting.hooks_mut())[..4], ["ffffff"; 4]);
    assert_eq!(shared.host_signals().ids().as_slice(), &[42]);
}

#[test]
fn a_comparison_on_a_host_signal_hears_a_first_value_of_zero() {
    let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
    let is_zero = condition_of(
        Logic::All,
        Logic::All,
        &[compare(
            Signal::Host(1),
            Comparator::Equal,
            Operand::Constant(0.0),
            None,
        )],
    );
    bar.configure(
        &profile_of([solid_on(0, 9, Some(is_zero))]),
        &Entries::new(),
        255,
    );
    assert_eq!(bar_colors(&mut bar)[0], "000000");
    bar.set_host_signals(&[HostSignal { id: 1, value: 0.0 }]);
    assert!(bar.needs_render(16));
    assert_eq!(bar_colors(&mut bar)[0], "ff0000");
}

#[test]
fn a_meter_far_over_its_maximum_stays_full() {
    let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
    let settings = profile_of([meter(Signal::Builtin(Builtin::KeyRate), 1.0, 0, 9)]);
    bar.configure(&settings, &Entries::new(), 255);
    for value in [25.0, 25.8, 1000.0] {
        bar.publish(Signal::Builtin(Builtin::KeyRate), value);
        assert_eq!(bar_colors(&mut bar), ["ffffff"; 10], "value {value}");
    }
}

#[test]
fn fast_scan_unbinds_a_comparison_with_a_key_signal_on_either_side() {
    let key_on_the_right = condition_of(
        Logic::All,
        Logic::All,
        &[compare(
            Signal::Builtin(Builtin::CapsLock),
            Comparator::NotEqual,
            Operand::Signal(Signal::Builtin(Builtin::KeyPressed)),
            None,
        )],
    );
    let key_on_the_left = condition_of(
        Logic::All,
        Logic::All,
        &[compare(
            Signal::Builtin(Builtin::KeyPressed),
            Comparator::NotEqual,
            Operand::Signal(Signal::Builtin(Builtin::CapsLock)),
            None,
        )],
    );
    let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
    let settings = profile_of([
        solid_on(0, 4, Some(key_on_the_right)),
        solid_on(5, 9, Some(key_on_the_left)),
    ]);
    bar.configure(&settings, &Entries::new(), 255);
    bar.set_fast_scan(true);
    let _ = bar_colors(&mut bar);
    bar.key_event(Key::new(2, 3), KeyEvent::Pressed);
    assert!(!bar.needs_render(16));
    assert_eq!(bar_colors(&mut bar), ["000000"; 10]);
}

#[test]
fn every_entry_hears_a_signal_whatever_the_logic_of_its_condition() {
    for (logic, group_logic) in [
        (Logic::Single, Logic::Single),
        (Logic::All, Logic::All),
        (Logic::Any, Logic::Any),
    ] {
        let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
        let global: Entries = (0..4)
            .map(|led| solid_on(led, led, Some(caps_is_on(logic, group_logic))))
            .collect();
        let settings =
            profile_of((4..8).map(|led| solid_on(led, led, Some(caps_is_on(logic, group_logic)))));
        bar.configure(&settings, &global, 255);
        bar.locks_changed(LockState {
            caps_lock: true,
            ..LockState::default()
        });
        assert_eq!(bar_colors(&mut bar)[..8], ["ff0000"; 8], "{logic:?}");
    }
}

#[test]
fn a_blink_hidden_by_its_condition_does_not_ask_to_render() {
    let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
    let hidden_blink = Entry {
        title: Title::new(),
        condition: Some(caps_is_on(Logic::All, Logic::All)),
        effect: Effect::Blink(Blink {
            from: color_of(rgb_of("ff0000")),
            to: color_of(rgb_of("0000ff")),
            period_ms: 1000,
            waveform: Waveform::Square,
            target: Target::Lightbar(Range { first: 0, last: 9 }),
        }),
    };
    bar.configure(&profile_of([hidden_blink]), &Entries::new(), 255);
    let mut frame = Frame::<BAR_PAGES, LENGTH>::new();
    let mut canvas = Painter::new(&mut frame, &BAR_MAP, &VARIANT, None);
    let mut asked = 0;
    for _ in 0..100 {
        if bar.needs_render(16) {
            asked += 1;
            bar.render(&mut canvas, 16);
        }
    }
    assert_eq!(asked, 1);
    bar.locks_changed(LockState {
        caps_lock: true,
        ..LockState::default()
    });
    assert!(bar.needs_render(16));
    bar.render(&mut canvas, 16);
    assert!(bar.needs_render(16));
}

#[test]
fn a_timed_comparison_asks_to_render_for_a_change_and_for_the_end_of_its_hold_only() {
    let timed = condition_of(
        Logic::All,
        Logic::All,
        &[compare(
            Signal::Builtin(Builtin::KeyRate),
            Comparator::Greater,
            Operand::Constant(5.0),
            Some(50),
        )],
    );
    let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
    bar.configure(
        &profile_of([solid_on(0, 9, Some(timed))]),
        &Entries::new(),
        255,
    );
    let mut frame = Frame::<BAR_PAGES, LENGTH>::new();
    let mut canvas = Painter::new(&mut frame, &BAR_MAP, &VARIANT, None);
    bar.publish(Signal::Builtin(Builtin::KeyRate), 6.0);
    bar.render(&mut canvas, 16);
    bar.publish(Signal::Builtin(Builtin::KeyRate), 7.0);
    assert!(bar.needs_render(16));
    bar.render(&mut canvas, 16);
    assert_eq!(canvas.led(BAR_LEDS[0]), Some(rgb_of("ff0000")));
    assert!(!bar.needs_render(16));
    assert!(bar.needs_render(40));
    bar.render(&mut canvas, 40);
    assert_eq!(canvas.led(BAR_LEDS[0]), Some(rgb_of("000000")));
    assert!(!bar.needs_render(16));
}

#[test]
fn the_mirror_and_a_transparent_meter_apply_the_board_gains_once() {
    let mirror = Entry {
        title: Title::new(),
        condition: None,
        effect: Effect::Mirror(Mirror {
            range: Range { first: 0, last: 9 },
            source: KeyRect {
                first: Position { row: 1, column: 0 },
                last: Position { row: 1, column: 9 },
            },
        }),
    };
    let mut bar = Bar::new(&BAR_LEDS, &GAINED_MAP, COLUMNS);
    bar.configure(&profile_of([mirror]), &Entries::new(), 255);
    let mut frame = Frame::<BAR_PAGES, LENGTH>::new();
    let mut canvas = Painter::new(&mut frame, &GAINED_MAP, &VARIANT, None);
    canvas.set_key(1, 0, rgb_of("ff0000"));
    bar.render(&mut canvas, 16);
    assert_eq!(canvas.key(1, 0), Some(rgb_of("e80000")));
    assert_eq!(canvas.led(BAR_LEDS[0]), Some(rgb_of("e80000")));

    let mut bar = Bar::new(&BAR_LEDS, &GAINED_MAP, COLUMNS);
    let mut global = Entries::new();
    global.push(solid_on(0, 0, None)).unwrap();
    let transparent = Entry {
        title: Title::new(),
        condition: None,
        effect: Effect::Meter(Meter {
            signal: Signal::Builtin(Builtin::KeyRate),
            maximum: 2.0,
            color: color_of(rgb_of("ffffff")),
            background: None,
            gradient_start: None,
            range: Range { first: 0, last: 0 },
            reverse: false,
        }),
    };
    bar.configure(&profile_of([transparent]), &global, 255);
    bar.publish(Signal::Builtin(Builtin::KeyRate), 1.0);
    bar.render(&mut canvas, 16);
    assert_eq!(
        canvas.led(BAR_LEDS[0]),
        Some(Rgb {
            red: 231,
            green: 126,
            blue: 126
        })
    );
}

#[test]
fn a_key_pressed_again_without_a_release_counts_once_and_rest_clears_it() {
    let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
    let settings = profile_of([
        meter(Signal::Builtin(Builtin::KeyPressed), 1.0, 0, 0),
        meter(Signal::Builtin(Builtin::KeyRate), 2.0, 1, 1),
    ]);
    bar.configure(&settings, &Entries::new(), 255);
    let key = Key::new(2, 3);
    bar.key_event(key, KeyEvent::Pressed);
    bar.configure(&settings, &Entries::new(), 255);
    bar.key_event(key, KeyEvent::Pressed);
    bar.travel_changed(2000);
    bar.tick(15);
    assert_eq!(bar_colors(&mut bar)[..2], ["ffffff", "7e7e7e"]);
    bar.travel_changed(0);
    assert_eq!(bar_colors(&mut bar)[..2], ["000000", "7e7e7e"]);
}

fn host_meter(id: u32, first: u8, last: u8) -> Entry {
    let mut entry = meter(Signal::Host(id), 1.0, first, last);
    if let Effect::Meter(meter) = &mut entry.effect {
        meter.background = Some(color_of(rgb_of("102030")));
    }
    entry
}

fn host_is_zero(id: u32) -> Condition {
    condition_of(
        Logic::All,
        Logic::All,
        &[compare(
            Signal::Host(id),
            Comparator::Equal,
            Operand::Constant(0.0),
            None,
        )],
    )
}

fn bar_lighting() -> Lighting<Bar, ROWS, COLUMNS, BAR_PAGES, LENGTH> {
    Lighting::new(
        &BAR_MAP,
        &VARIANT,
        GEOMETRY,
        Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS),
        0,
    )
}

#[test]
fn a_host_session_end_zeroes_the_host_signals_and_darkens_host_meters_until_the_host_returns() {
    let shared = profile::Shared::<CriticalSectionRawMutex>::new();
    let mut lighting = bar_lighting();
    shared.edit(|appearance| {
        appearance.lightbar =
            profile_of([host_meter(42, 0, 3), solid_on(4, 9, Some(host_is_zero(7)))]);
    });
    shared.activate(false);
    shared.host_signals().set(&[
        HostSignal { id: 42, value: 1.0 },
        HostSignal { id: 7, value: 2.0 },
        HostSignal { id: 99, value: 1.0 },
    ]);
    shared.apply(&mut lighting, 0);
    let shown = bar_colors(lighting.hooks_mut());
    assert_eq!(shown[..4], ["ffffff"; 4]);
    assert_eq!(shown[4..], ["000000"; 6]);

    shared
        .host_signals()
        .set(&[HostSignal { id: 42, value: 0.5 }]);
    shared.host_signals().end_session();
    shared.apply(&mut lighting, 4);
    assert!(lighting.hooks().needs_render(0));
    let ended = bar_colors(lighting.hooks_mut());
    assert_eq!(
        ended[..4],
        ["000000"; 4],
        "the host meter is off and the value 0.5 queued before the end was dropped"
    );
    assert_eq!(
        ended[4..],
        ["ff0000"; 6],
        "the condition sees host signal 7 at 0"
    );

    assert_eq!(shared.host_signals().ids().as_slice(), &[42, 7]);
    shared.apply(&mut lighting, 8);
    assert_eq!(
        bar_colors(lighting.hooks_mut())[..4],
        ["102030"; 4],
        "listing the signals turns the meter on again at the value 0"
    );

    shared.change(profile::Changes::LIGHTBAR, |appearance| {
        appearance.lightbar = profile_of([host_meter(99, 0, 3)]);
    });
    shared.apply(&mut lighting, 12);
    assert_eq!(
        bar_colors(lighting.hooks_mut())[..4],
        ["102030"; 4],
        "the value kept for the unbound id 99 was zeroed"
    );
    shared
        .host_signals()
        .set(&[HostSignal { id: 99, value: 1.0 }]);
    shared.apply(&mut lighting, 16);
    assert_eq!(bar_colors(lighting.hooks_mut())[..4], ["ffffff"; 4]);
}

#[test]
fn a_host_session_end_before_any_host_value_lets_host_conditions_see_zero() {
    let mut bar = Bar::new(&BAR_LEDS, &BAR_MAP, COLUMNS);
    bar.configure(
        &profile_of([solid_on(0, 9, Some(host_is_zero(1)))]),
        &Entries::new(),
        255,
    );
    assert_eq!(bar_colors(&mut bar)[0], "000000");
    bar.end_host_signals();
    assert!(bar.needs_render(16));
    assert_eq!(bar_colors(&mut bar)[0], "ff0000");
}

mod bus_reset {
    use std::pin::pin;

    use embassy_futures::block_on;
    use embassy_sync::mutex::Mutex;
    use embassy_usb_driver::Event;
    use openwoot::{
        diagnostics::Diagnostics,
        lighting::{HostControl, KeyEvents, LedMatrix},
        protocol::{Channel, Handler},
        runtime,
        usb::{Face, HostState, Resources, build},
    };
    use openwoot_protocol::{
        FirmwareVersion, PROTOCOL_VERSION, Version, encode_request, endpoints::GetVersion, report,
    };

    use super::*;
    use crate::support::{Host, IDENTITY, Outcome, Recorder, poll, setup, stock_identity};

    const CHANNEL_OUT: u8 = 0x03;
    const PACKET_SIZE: usize = 64;
    const SET_CONFIGURATION: u8 = 0x09;
    const VERSION: Version = Version {
        protocol: PROTOCOL_VERSION,
        firmware: FirmwareVersion {
            major: 1,
            minor: 2,
            patch: 3,
        },
    };

    struct NoLeds;

    #[expect(
        clippy::unused_async_trait_impl,
        reason = "the test never lights the chip"
    )]
    impl LedMatrix for NoLeds {
        type Error = ();

        async fn initialize(&mut self) -> Result<(), ()> {
            Ok(())
        }

        async fn set_running(&mut self, _running: bool) -> Result<(), ()> {
            Ok(())
        }

        async fn write(&mut self, _bank: u8, _first: u8, _values: &[u8]) -> Result<(), ()> {
            Ok(())
        }
    }

    #[test]
    fn a_bus_reset_zeroes_the_host_signals_and_turns_host_meters_off() {
        static STATE: HostState = HostState::new();
        static HOST_CONTROL: HostControl = HostControl::new();
        let host = Host::default();
        let mut resources = Resources::new();
        let mut device = build(
            host.driver(),
            Face::default(),
            IDENTITY,
            &mut resources,
            &STATE,
        );
        let mut channel = Channel::new(device.channel.output, device.channel.input);
        let mut run = pin!(device.usb.run());
        host.attach(run.as_mut());
        let configure = setup(0, SET_CONFIGURATION, 1, 0, 0);
        assert_eq!(
            host.control_out(run.as_mut(), configure, &[]),
            Some(Outcome::Accepted)
        );
        let mut handler = Handler::new(VERSION, stock_identity(None))
            .with_host(&STATE)
            .with_host_control(&HOST_CONTROL);
        let mut keyboard = Recorder::default();

        let shared = profile::Shared::<CriticalSectionRawMutex>::new();
        shared.edit(|appearance| {
            appearance.lightbar =
                profile_of([host_meter(42, 0, 3), solid_on(4, 9, Some(host_is_zero(7)))]);
        });
        shared.activate(false);
        let lighting = Mutex::<CriticalSectionRawMutex, _>::new(bar_lighting());
        let events = KeyEvents::new();
        let notices = runtime::KeyNotices::new();
        let diagnostics = Diagnostics::new();
        let task = runtime::Lighting {
            lighting: &lighting,
            events: &events,
            notices: &notices,
            host: &STATE,
            host_control: &HOST_CONTROL,
            diagnostics: &diagnostics,
            profile: &shared,
        };
        let shown = || bar_colors(block_on(lighting.lock()).hooks_mut());

        shared.host_signals().set(&[
            HostSignal { id: 42, value: 1.0 },
            HostSignal { id: 7, value: 2.0 },
        ]);
        block_on(task.step(&mut NoLeds, &(), 0, false));
        assert_eq!(shown()[..4], ["ffffff"; 4]);
        assert_eq!(shown()[4..], ["000000"; 6]);

        host.bus_events(run.as_mut(), &[Event::Reset]);
        assert_eq!(
            host.control_out(run.as_mut(), configure, &[]),
            Some(Outcome::Accepted)
        );
        let mut request = [0; report::MAX_LEN];
        let request = encode_request::<GetVersion>(2, &(), &mut request).unwrap();
        host.send_packets(CHANNEL_OUT, request, PACKET_SIZE);
        assert_eq!(
            poll(pin!(channel.serve(
                &mut handler,
                STATE.feature_requests(),
                &mut keyboard
            ))),
            Some(Ok(()))
        );
        block_on(task.step(&mut NoLeds, &(), 4, false));
        assert_eq!(shown()[..4], ["000000"; 4], "the host meter is off");
        assert_eq!(shown()[4..], ["ff0000"; 6], "host signal 7 reads 0");

        shared
            .host_signals()
            .set(&[HostSignal { id: 42, value: 1.0 }]);
        block_on(task.step(&mut NoLeds, &(), 8, false));
        assert_eq!(
            shown()[..4],
            ["ffffff"; 4],
            "a new value turns the meter on again"
        );
    }
}
