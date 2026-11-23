use openwoot::{
    keys::Key,
    lighting::{
        FramePage, Gains, Geometry, Led, LedMap, LedPosition, Lighting, Variant, from_rgb565, rgb,
    },
};
use openwoot_protocol::lighting::{
    AnalogLevel, AnalogRgbFill, Breathing, Color, Effect, Effects, HueCycle, HueRange, Indicators,
    KeyColors, KeyRipple, KeyTest, LevelMeter, PolarGradient, RainbowWave, ReactiveColoring,
    ReactiveFade, Rgb, Scanner, ScannerDirection, Settings, SimplexNoise,
};

pub(crate) const ROWS: usize = 6;
pub(crate) const COLUMNS: usize = 17;
pub(crate) const PAGES: usize = 9;
pub(crate) const LENGTH: usize = 36;
pub(crate) const START_MS: u32 = 1000;
const LEDS_PER_PAGE: usize = LENGTH / 3;
const LED_COUNT: usize = ROWS * COLUMNS + 1;
const HOLES: [(usize, usize); 14] = [
    (0, 9),
    (1, 13),
    (2, 13),
    (3, 12),
    (3, 13),
    (3, 15),
    (3, 16),
    (4, 1),
    (4, 12),
    (4, 13),
    (4, 16),
    (5, 3),
    (5, 9),
    (5, 13),
];

pub(crate) static MAP: LedMap<PAGES> = LedMap {
    positions: &POSITIONS,
    pages: pages(),
    gains: Gains::FULL,
};

pub(crate) static VARIANT: Variant<ROWS, COLUMNS> = Variant {
    leds: leds(),
    spacebar: Key::new(5, 6),
    spacebar_fill: &[4, 5, 7, 8],
};

pub(crate) const GEOMETRY: Geometry = Geometry {
    first_key_row: 0,
    level_meter_segments: 10,
};

static POSITIONS: [Option<LedPosition>; LED_COUNT] = positions();

pub(crate) type TestLighting = Lighting<(), ROWS, COLUMNS, PAGES, LENGTH>;

#[derive(PartialEq, Eq)]
pub(crate) struct Chip {
    registers: [[u8; 256]; 2],
}

impl Chip {
    pub(crate) fn new() -> Self {
        Self {
            registers: [[0; 256]; 2],
        }
    }

    pub(crate) fn drain(&mut self, lighting: &mut TestLighting) -> usize {
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

    pub(crate) fn key(&self, row: usize, column: usize) -> Rgb {
        let Some(position) = VARIANT.led(row, column).and_then(|led| MAP.position(led)) else {
            return Rgb::default();
        };
        let page = MAP.page(usize::from(position.page)).unwrap();
        let register = usize::from(page.first_register) + usize::from(position.offset);
        let channels = &self.registers[usize::from(page.bank)][register..register + 3];
        rgb(channels[0], channels[1], channels[2])
    }

    pub(crate) fn digest(&self) -> u64 {
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

pub(crate) fn lighting(brightness: u8, effects: Effects) -> TestLighting {
    let mut lighting = Lighting::new(&MAP, &VARIANT, GEOMETRY, (), START_MS);
    lighting.apply_settings(&settings(brightness, effects), START_MS);
    lighting.set_base_colors(&key_colors(base_color));
    lighting.set_layer_colors(
        1,
        Some(&key_colors(|row, column| base_color(column, row + 3))),
    );
    lighting
}

pub(crate) fn settle(lighting: &mut TestLighting, chip: &mut Chip, from_ms: u32, to_ms: u32) {
    for now_ms in from_ms..=to_ms {
        lighting.update(now_ms, &());
        chip.drain(lighting);
    }
}

pub(crate) const fn base_color(row: usize, column: usize) -> u16 {
    ((((row * 37 + column * 11) & 0x1F) << 11)
        | (((row * 13 + column * 29) & 0x3F) << 5)
        | ((row * 7 + column * 3) & 0x1F)) as u16
}

pub(crate) fn unpack(packed: u32) -> Color {
    let [alpha, red, green, blue] = packed.to_be_bytes();
    if packed & 0x8000_0000 == 0 {
        let rgb = from_rgb565(packed as u16);
        Color {
            red: rgb.red,
            green: rgb.green,
            blue: rgb.blue,
            alpha: u8::MAX,
        }
    } else {
        Color {
            red,
            green,
            blue,
            alpha: (alpha & 0x7F) << 1,
        }
    }
}

pub(crate) fn byte(value: u32) -> u8 {
    u8::try_from(value).unwrap()
}

pub(crate) fn effect(kind: u32, parameters: [u32; 4]) -> Effect {
    let [first, second, third, fourth] = parameters;
    match kind {
        1 => Effect::RainbowWave(RainbowWave {
            hues: hues(first),
            period: byte(second),
            forward: flag(third),
        }),
        2 => Effect::Scanner(Scanner {
            width: byte(first),
            steps_per_second: byte(second),
            outside_level: byte(third),
            direction: direction(fourth),
        }),
        3 => Effect::Breathing(Breathing {
            step_ms: byte(first),
            min_level: byte(second),
        }),
        4 => Effect::HueCycle(HueCycle {
            hues: hues(first),
            period: byte(second),
        }),
        5 if first & !0b101 != 0 => panic!("unknown ripple flags {first:#x} in the golden data"),
        5 => Effect::KeyRipple(KeyRipple {
            palette: first & 1 != 0,
            inverted: first & 4 != 0,
            steps_per_second: byte(second),
            decay_step_ms: byte(third),
            base_level: byte(fourth),
        }),
        6 if third & !0b111 != 0 || third & 0b11 == 0b11 => {
            panic!("unknown reactive flags {third:#x} in the golden data")
        }
        6 => Effect::ReactiveFade(ReactiveFade {
            base_level: byte(first),
            step_ms: byte(second),
            coloring: if third & 1 != 0 {
                ReactiveColoring::Palette
            } else if third & 2 != 0 {
                ReactiveColoring::Fixed(unpack(fourth))
            } else {
                ReactiveColoring::KeyColor
            },
            inverted: third & 4 != 0,
        }),
        7 => Effect::LevelMeter(LevelMeter {
            base_level: byte(first),
            gamepad_curve: flag(second),
        }),
        8 => Effect::AnalogSpotlight(AnalogLevel {
            base_level: byte(first),
            gamepad_curve: flag(second),
        }),
        9 => Effect::AnalogGlow(AnalogLevel {
            base_level: byte(first),
            gamepad_curve: flag(second),
        }),
        10 => Effect::AnalogRgbFill(AnalogRgbFill {
            gamepad_curve: flag(first),
        }),
        11 => Effect::KeyTest(KeyTest {
            base_level: byte(first),
        }),
        14 => Effect::SimplexNoise(SimplexNoise {
            from: unpack(first),
            to: unpack(second),
            speed: byte(third),
            scale: byte(fourth),
        }),
        15 => Effect::PolarGradient(PolarGradient {
            from: unpack(first),
            to: unpack(second),
            speed: i8::try_from(third.cast_signed()).unwrap(),
            twist: i8::try_from(fourth.cast_signed()).unwrap(),
        }),
        other => panic!("no effect kind {other} in the golden data"),
    }
}

const fn positions() -> [Option<LedPosition>; LED_COUNT] {
    let mut positions = [None; LED_COUNT];
    let mut led = 1;
    while led < LED_COUNT {
        let index = led - 1;
        positions[led] = Some(LedPosition::new(
            (index / LEDS_PER_PAGE) as u8,
            (index % LEDS_PER_PAGE * 3) as u8,
        ));
        led += 1;
    }
    positions
}

const fn pages() -> [FramePage; PAGES] {
    let mut pages = [FramePage::new(0, 0, LENGTH as u8); PAGES];
    let mut page = 0;
    while page < PAGES {
        let first = page * LENGTH;
        pages[page] = FramePage::new((first / 180) as u8, (first % 180) as u8, LENGTH as u8);
        page += 1;
    }
    pages
}

const fn leds() -> [[Option<Led>; COLUMNS]; ROWS] {
    let mut leds = [[None; COLUMNS]; ROWS];
    let mut row = 0;
    while row < ROWS {
        let mut column = 0;
        while column < COLUMNS {
            if !is_hole(row, column) {
                leds[row][column] = Some(Led((row * COLUMNS + column + 1) as u8));
            }
            column += 1;
        }
        row += 1;
    }
    leds
}

const fn is_hole(row: usize, column: usize) -> bool {
    let mut index = 0;
    while index < HOLES.len() {
        if HOLES[index].0 == row && HOLES[index].1 == column {
            return true;
        }
        index += 1;
    }
    false
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

fn hues(code: u32) -> HueRange {
    match code {
        0 => HueRange::FullCircle,
        1 => HueRange::From110To184,
        2 => HueRange::From2To39,
        3 => HueRange::From64To114,
        4 => HueRange::From214To254,
        5 => HueRange::From113To204,
        other => panic!("no hue range {other} in the golden data"),
    }
}

fn direction(code: u32) -> ScannerDirection {
    match code {
        0 => ScannerDirection::Left,
        1 => ScannerDirection::Right,
        2 => ScannerDirection::Up,
        3 => ScannerDirection::Down,
        other => panic!("no scanner direction {other} in the golden data"),
    }
}

fn flag(value: u32) -> bool {
    match value {
        0 => false,
        1 => true,
        other => panic!("flag {other} is neither 0 nor 1 in the golden data"),
    }
}
