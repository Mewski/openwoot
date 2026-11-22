use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::configuration::{Capabilities, Invalid, MAX_KEYS, Validate};

pub const MAX_EFFECTS: usize = 6;

pub type Effects = Vec<Effect, MAX_EFFECTS>;
pub type KeyColors = Vec<Rgb, MAX_KEYS>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Rgb {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Settings {
    pub brightness: u8,
    pub indicators: Indicators,
    pub effects: Effects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Indicators {
    pub caps_lock: Color,
    pub scroll_lock: Color,
    pub num_lock: Color,
    pub layer_lock: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Idle {
    pub dim_after_minutes: u8,
    pub off_after_minutes: u8,
    pub off_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Effect {
    RainbowWave(RainbowWave),
    Scanner(Scanner),
    Breathing(Breathing),
    HueCycle(HueCycle),
    KeyRipple(KeyRipple),
    ReactiveFade(ReactiveFade),
    LevelMeter(LevelMeter),
    AnalogSpotlight(AnalogLevel),
    AnalogGlow(AnalogLevel),
    AnalogRgbFill(AnalogRgbFill),
    KeyTest(KeyTest),
    SimplexNoise(SimplexNoise),
    PolarGradient(PolarGradient),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum HueRange {
    #[default]
    FullCircle,
    From110To184,
    From2To39,
    From64To114,
    From214To254,
    From113To204,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct RainbowWave {
    pub hues: HueRange,
    pub period: u8,
    pub forward: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Scanner {
    pub width: u8,
    pub steps_per_second: u8,
    pub outside_level: u8,
    pub direction: ScannerDirection,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum ScannerDirection {
    #[default]
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Breathing {
    pub step_ms: u8,
    pub min_level: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct HueCycle {
    pub hues: HueRange,
    pub period: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct KeyRipple {
    pub palette: bool,
    pub inverted: bool,
    pub steps_per_second: u8,
    pub decay_step_ms: u8,
    pub base_level: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct ReactiveFade {
    pub base_level: u8,
    pub step_ms: u8,
    pub coloring: ReactiveColoring,
    pub inverted: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum ReactiveColoring {
    #[default]
    KeyColor,
    Palette,
    Fixed(Color),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct LevelMeter {
    pub base_level: u8,
    pub gamepad_curve: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct AnalogLevel {
    pub base_level: u8,
    pub gamepad_curve: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct AnalogRgbFill {
    pub gamepad_curve: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct KeyTest {
    pub base_level: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct SimplexNoise {
    pub from: Color,
    pub to: Color,
    pub speed: u8,
    pub scale: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct PolarGradient {
    pub from: Color,
    pub to: Color,
    pub speed: i8,
    pub twist: i8,
}

impl Rgb {
    #[must_use]
    pub const fn opaque(self) -> Color {
        Color {
            red: self.red,
            green: self.green,
            blue: self.blue,
            alpha: Color::OPAQUE,
        }
    }
}

impl Color {
    pub const OPAQUE: u8 = u8::MAX;
    pub const TRANSPARENT: Self = Self {
        red: 0,
        green: 0,
        blue: 0,
        alpha: 0,
    };

    #[must_use]
    pub const fn rgb(self) -> Rgb {
        Rgb {
            red: self.red,
            green: self.green,
            blue: self.blue,
        }
    }
}

impl Validate for Effect {
    fn validate(&self, _: &Capabilities) -> Result<(), Invalid> {
        match self {
            Self::RainbowWave(RainbowWave { period, .. })
            | Self::HueCycle(HueCycle { period, .. }) => in_range(*period, 1, u8::MAX),
            Self::Scanner(scanner) => in_range(scanner.steps_per_second, 1, 60),
            Self::Breathing(breathing) => in_range(breathing.step_ms, 1, 40),
            Self::KeyRipple(ripple) => {
                in_range(ripple.steps_per_second, 1, 50)?;
                in_range(ripple.decay_step_ms, 1, u8::MAX)
            }
            Self::ReactiveFade(fade) => in_range(fade.step_ms, 1, 40),
            Self::LevelMeter(_)
            | Self::AnalogSpotlight(_)
            | Self::AnalogGlow(_)
            | Self::AnalogRgbFill(_)
            | Self::KeyTest(_)
            | Self::SimplexNoise(_)
            | Self::PolarGradient(_) => Ok(()),
        }
    }
}

impl Validate for Settings {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.effects
            .iter()
            .try_for_each(|effect| effect.validate(capabilities))
    }
}

impl Validate for KeyColors {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        capabilities.check_key_count(self.len())
    }
}

fn in_range(value: u8, min: u8, max: u8) -> Result<(), Invalid> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(Invalid::RateOutsideRange { value, min, max })
    }
}
