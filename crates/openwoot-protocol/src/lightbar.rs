use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::{
    configuration::{Capabilities, Invalid, Validate},
    keys::Position,
    lighting::{Color, Rgb},
};

pub const MAX_ENTRIES: usize = 4;
pub const MAX_GROUPS: usize = 2;
pub const MAX_TERMS: usize = 2;
pub const MAX_LEDS: usize = 16;
pub const MAX_HOST_SIGNALS: usize = 20;

pub type Entries = Vec<Entry, MAX_ENTRIES>;
pub type HostSignals = Vec<HostSignal, MAX_HOST_SIGNALS>;
pub type HostSignalIds = Vec<u32, MAX_HOST_SIGNALS>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
pub struct Settings {
    pub enabled: bool,
    pub entries: Entries,
    pub use_global: bool,
    pub brightness: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
pub struct Entry {
    pub condition: Option<Condition>,
    pub effect: Effect,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
pub struct Condition {
    pub logic: Logic,
    pub groups: Vec<Group, MAX_GROUPS>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
pub struct Group {
    pub logic: Logic,
    pub terms: Vec<Term, MAX_TERMS>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Logic {
    #[default]
    Single,
    All,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Schema)]
pub enum Term {
    Trigger { signal: Signal, duration_ms: u16 },
    Compare(Comparison),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Schema)]
pub struct Comparison {
    pub signal: Signal,
    pub comparator: Comparator,
    pub operand: Operand,
    pub hold_ms: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Comparator {
    Equal,
    NotEqual,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Schema)]
pub enum Operand {
    Signal(Signal),
    Constant(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Signal {
    Builtin(Builtin),
    Host(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Builtin {
    ActiveProfile,
    AnalogLevel,
    KeyPressed,
    CapsLock,
    ScrollLock,
    NumLock,
    ProfileCount,
    ActiveLayer,
    Brightness,
    KeyRate,
    ProfileColor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Schema)]
pub enum Effect {
    Meter(Meter),
    ProfileIndicator(ProfileIndicator),
    Mirror(Mirror),
    Blink(Blink),
    Solid(Solid),
    Custom(Custom),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Schema)]
pub struct Meter {
    pub signal: Signal,
    pub maximum: f32,
    pub color: Color,
    pub background: Option<Color>,
    pub gradient_start: Option<Color>,
    pub range: Range,
    pub reverse: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct ProfileIndicator {
    pub range: Range,
    pub background: Background,
    pub segments: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Background {
    #[default]
    ProfileColor,
    Transparent,
    Fixed(Color),
    Contrast,
    DimmedProfileColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Mirror {
    pub range: Range,
    pub source: KeyRect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Blink {
    pub from: Color,
    pub to: Color,
    pub period_ms: u16,
    pub waveform: Waveform,
    pub target: Target,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Waveform {
    #[default]
    Square,
    Triangle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Solid {
    pub color: Color,
    pub target: Target,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Custom {
    pub colors: Vec<Rgb, MAX_LEDS>,
    pub range: Range,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Target {
    Lightbar(Range),
    Keys(KeyRect),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Range {
    pub first: u8,
    pub last: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct KeyRect {
    pub first: Position,
    pub last: Position,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Schema)]
pub struct HostSignal {
    pub id: u32,
    pub value: f32,
}

impl Validate for Range {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        if self.first <= self.last && self.last < capabilities.lightbar_leds {
            Ok(())
        } else {
            Err(Invalid::LightbarRangeOutsideLeds {
                first: self.first,
                last: self.last,
                leds: capabilities.lightbar_leds,
            })
        }
    }
}

impl Validate for KeyRect {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.first.validate(capabilities)?;
        self.last.validate(capabilities)
    }
}

impl Validate for Target {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        match self {
            Self::Lightbar(range) => range.validate(capabilities),
            Self::Keys(rect) => rect.validate(capabilities),
        }
    }
}

impl Validate for Signal {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        match self {
            Self::Host(_) if !capabilities.host_signals => Err(Invalid::NoHostSignals),
            Self::Builtin(_) | Self::Host(_) => Ok(()),
        }
    }
}

impl Validate for Term {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        match self {
            Self::Trigger { signal, .. } => signal.validate(capabilities),
            Self::Compare(comparison) => {
                comparison.signal.validate(capabilities)?;
                match comparison.operand {
                    Operand::Signal(signal) => signal.validate(capabilities),
                    Operand::Constant(_) => Ok(()),
                }
            }
        }
    }
}

impl Validate for Effect {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        match self {
            Self::Meter(meter) => {
                meter.signal.validate(capabilities)?;
                if !(meter.maximum.is_finite() && meter.maximum > 0.0) {
                    return Err(Invalid::MeterMaximumNotPositive);
                }
                meter.range.validate(capabilities)
            }
            Self::ProfileIndicator(indicator) => {
                if indicator.segments == 0 {
                    return Err(Invalid::ZeroSegments);
                }
                indicator.range.validate(capabilities)
            }
            Self::Mirror(mirror) => {
                mirror.range.validate(capabilities)?;
                mirror.source.validate(capabilities)
            }
            Self::Blink(blink) => {
                if blink.period_ms == 0 {
                    return Err(Invalid::ZeroPeriod);
                }
                blink.target.validate(capabilities)
            }
            Self::Solid(solid) => solid.target.validate(capabilities),
            Self::Custom(custom) => {
                custom.range.validate(capabilities)?;
                if custom.colors.len() > usize::from(capabilities.lightbar_leds) {
                    return Err(Invalid::LightbarColorsExceedLeds {
                        count: u8::try_from(custom.colors.len()).unwrap_or(u8::MAX),
                        leds: capabilities.lightbar_leds,
                    });
                }
                Ok(())
            }
        }
    }
}

impl Validate for Condition {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.groups
            .iter()
            .flat_map(|group| group.terms.iter())
            .try_for_each(|term| term.validate(capabilities))
    }
}

impl Validate for Entry {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.condition.validate(capabilities)?;
        self.effect.validate(capabilities)
    }
}

impl Validate for Entries {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        if capabilities.lightbar_leds == 0 {
            return Err(Invalid::NoLightbar);
        }
        self.iter()
            .try_for_each(|entry| entry.validate(capabilities))
    }
}

impl Validate for Settings {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        self.entries.validate(capabilities)
    }
}

impl Validate for HostSignals {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        if capabilities.host_signals {
            Ok(())
        } else {
            Err(Invalid::NoHostSignals)
        }
    }
}
