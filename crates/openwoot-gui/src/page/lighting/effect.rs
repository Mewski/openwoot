use std::mem;

use gpui_kit::assets::IconName as Catalog;
use openwoot_protocol::{
    configuration::Capabilities,
    lighting::{
        AnalogLevel, AnalogRgbFill, Breathing, Color, Effect, HueCycle, HueRange, KeyRipple,
        KeyTest, LevelMeter, PolarGradient, RainbowWave, ReactiveColoring, ReactiveFade, Scanner,
        ScannerDirection, SimplexNoise,
    },
};

pub(super) const BASE_KINDS: [Effect; 4] = [WAVE, CYCLE, NOISE, SPIRAL];
pub(super) const LAYER_KINDS: [Effect; 9] = [
    Effect::AnalogSpotlight(AnalogLevel {
        base_level: 30,
        gamepad_curve: false,
    }),
    SCAN,
    Effect::Breathing(Breathing {
        step_ms: 15,
        min_level: 0,
    }),
    Effect::KeyRipple(KeyRipple {
        palette: false,
        inverted: false,
        steps_per_second: 20,
        decay_step_ms: 30,
        base_level: 30,
    }),
    Effect::LevelMeter(LevelMeter {
        base_level: 30,
        gamepad_curve: false,
    }),
    Effect::AnalogGlow(AnalogLevel {
        base_level: 30,
        gamepad_curve: false,
    }),
    Effect::AnalogRgbFill(AnalogRgbFill {
        gamepad_curve: false,
    }),
    Effect::ReactiveFade(ReactiveFade {
        base_level: 30,
        step_ms: 4,
        coloring: ReactiveColoring::KeyColor,
        inverted: false,
    }),
    Effect::KeyTest(KeyTest { base_level: 140 }),
];
pub(super) const SCAN: Effect = Effect::Scanner(Scanner {
    width: 1,
    steps_per_second: 15,
    outside_level: 30,
    direction: ScannerDirection::Left,
});
pub(super) const HUE_RANGES: [(HueRange, &str); 6] = [
    (HueRange::FullCircle, "Rainbow"),
    (HueRange::From110To184, "Ocean"),
    (HueRange::From2To39, "Inferno"),
    (HueRange::From64To114, "Spring"),
    (HueRange::From214To254, "Pink!"),
    (HueRange::From113To204, "Blue Spectrum"),
];

const WAVE: Effect = Effect::RainbowWave(RainbowWave {
    hues: HueRange::FullCircle,
    period: 16,
    forward: false,
});
const CYCLE: Effect = Effect::HueCycle(HueCycle {
    hues: HueRange::FullCircle,
    period: 48,
});
const NOISE: Effect = Effect::SimplexNoise(SimplexNoise {
    from: color(30, 0, 255),
    to: color(0, 255, 10),
    speed: 50,
    scale: 25,
});
const SPIRAL: Effect = Effect::PolarGradient(PolarGradient {
    from: color(255, 0, 255),
    to: color(0, 255, 255),
    speed: 50,
    twist: -70,
});
const PERIOD_SECONDS: f32 = 0.25;
const BREATH_STEP_SECONDS: f32 = 0.252;
const FADE_STEP_SECONDS: f32 = 0.015;
const TRAIL_STEP_SECONDS: f32 = 0.25;
const LEVEL_MAX: f32 = 255.0;
const PERCENT_MAX: f32 = 100.0;
const SIGNED_MAX: f32 = 127.0;
const STEP_MS_MAX: f32 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dial {
    Period,
    Width,
    ScanSpeed,
    BreathSpeed,
    SpreadSpeed,
    FadeSpeed,
    TrailSpeed,
    Level,
    NoiseSpeed,
    NoiseScale,
    Spin,
    Twist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Paint {
    KeyColor,
    Palette,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Turn {
    Spin,
    Twist,
}

pub(super) const fn name(effect: &Effect) -> &'static str {
    match effect {
        Effect::RainbowWave(_) => "Wave",
        Effect::Scanner(_) => "Scan",
        Effect::Breathing(_) => "Breath",
        Effect::HueCycle(_) => "Cycle",
        Effect::KeyRipple(_) => "Ripple",
        Effect::ReactiveFade(_) => "Trail",
        Effect::LevelMeter(_) => "Touch",
        Effect::AnalogSpotlight(_) => "Jelly",
        Effect::AnalogGlow(_) => "AOE",
        Effect::AnalogRgbFill(_) => "Mixing",
        Effect::KeyTest(_) => "RT display",
        Effect::SimplexNoise(_) => "Noise",
        Effect::PolarGradient(_) => "Spiral",
    }
}

pub(super) const fn description(effect: &Effect) -> &'static str {
    match effect {
        Effect::RainbowWave(_) => "A band of colours moves across the keys",
        Effect::Scanner(_) => "A bar of light sweeps across the keys",
        Effect::Breathing(_) => "All keys fade down and up",
        Effect::HueCycle(_) => "All keys change colour together",
        Effect::KeyRipple(_) => "Key presses send out ripples",
        Effect::ReactiveFade(_) => "Pressed keys light up and fade out",
        Effect::LevelMeter(_) => "The number row shows how deep the deepest key is",
        Effect::AnalogSpotlight(_) => "Keys around a pressed key light up with its depth",
        Effect::AnalogGlow(_) => "Pressed keys and their neighbours glow with the depth",
        Effect::AnalogRgbFill(_) => "The three bottom-right keys mix red, green and blue",
        Effect::KeyTest(_) => "Shows the Rapid Trigger state of each key",
        Effect::SimplexNoise(_) => "Two colours drift across the keys like clouds",
        Effect::PolarGradient(_) => "Two colours swirl around the centre",
    }
}

pub(super) const fn icon(effect: &Effect) -> Catalog {
    match effect {
        Effect::RainbowWave(_) => Catalog::WavesHorizontal,
        Effect::Scanner(_) => Catalog::ScanLine,
        Effect::Breathing(_) => Catalog::Wind,
        Effect::HueCycle(_) => Catalog::RefreshCw,
        Effect::KeyRipple(_) => Catalog::Grip,
        Effect::ReactiveFade(_) => Catalog::Footprints,
        Effect::LevelMeter(_) => Catalog::Ellipsis,
        Effect::AnalogSpotlight(_) => Catalog::Atom,
        Effect::AnalogGlow(_) => Catalog::Radar,
        Effect::AnalogRgbFill(_) => Catalog::SlidersVertical,
        Effect::KeyTest(_) => Catalog::Zap,
        Effect::SimplexNoise(_) => Catalog::AudioLines,
        Effect::PolarGradient(_) => Catalog::Shell,
    }
}

pub(super) const fn is_base(effect: &Effect) -> bool {
    matches!(
        effect,
        Effect::RainbowWave(_)
            | Effect::HueCycle(_)
            | Effect::SimplexNoise(_)
            | Effect::PolarGradient(_)
    )
}

pub(super) fn same_kind(effect: &Effect, other: &Effect) -> bool {
    mem::discriminant(effect) == mem::discriminant(other)
}

pub(super) const fn hue_range(effect: &Effect) -> Option<HueRange> {
    match effect {
        Effect::RainbowWave(wave) => Some(wave.hues),
        Effect::HueCycle(cycle) => Some(cycle.hues),
        _ => None,
    }
}

pub(super) const fn set_hues(effect: &mut Effect, hues: HueRange) {
    match effect {
        Effect::RainbowWave(wave) => wave.hues = hues,
        Effect::HueCycle(cycle) => cycle.hues = hues,
        _ => {}
    }
}

pub(super) const fn colors(effect: &Effect) -> (Option<Color>, Option<Color>) {
    match effect {
        Effect::SimplexNoise(noise) => (Some(noise.from), Some(noise.to)),
        Effect::PolarGradient(gradient) => (Some(gradient.from), Some(gradient.to)),
        Effect::ReactiveFade(ReactiveFade {
            coloring: ReactiveColoring::Fixed(fixed),
            ..
        }) => (Some(*fixed), None),
        _ => (None, None),
    }
}

pub(super) const fn set_first_color(effect: &mut Effect, color: Color) {
    match effect {
        Effect::SimplexNoise(noise) => noise.from = color,
        Effect::PolarGradient(gradient) => gradient.from = color,
        Effect::ReactiveFade(fade) => fade.coloring = ReactiveColoring::Fixed(color),
        _ => {}
    }
}

pub(super) const fn set_second_color(effect: &mut Effect, color: Color) {
    match effect {
        Effect::SimplexNoise(noise) => noise.to = color,
        Effect::PolarGradient(gradient) => gradient.to = color,
        _ => {}
    }
}

pub(super) const fn analog_curve(effect: &Effect) -> Option<bool> {
    match effect {
        Effect::LevelMeter(LevelMeter { gamepad_curve, .. })
        | Effect::AnalogSpotlight(AnalogLevel { gamepad_curve, .. })
        | Effect::AnalogGlow(AnalogLevel { gamepad_curve, .. })
        | Effect::AnalogRgbFill(AnalogRgbFill { gamepad_curve }) => Some(*gamepad_curve),
        _ => None,
    }
}

pub(super) const fn set_analog_curve(effect: &mut Effect, enabled: bool) {
    match effect {
        Effect::LevelMeter(LevelMeter { gamepad_curve, .. })
        | Effect::AnalogSpotlight(AnalogLevel { gamepad_curve, .. })
        | Effect::AnalogGlow(AnalogLevel { gamepad_curve, .. })
        | Effect::AnalogRgbFill(AnalogRgbFill { gamepad_curve }) => *gamepad_curve = enabled,
        _ => {}
    }
}

pub(super) const fn paint_of(coloring: ReactiveColoring) -> Paint {
    match coloring {
        ReactiveColoring::KeyColor => Paint::KeyColor,
        ReactiveColoring::Palette => Paint::Palette,
        ReactiveColoring::Fixed(_) => Paint::Fixed,
    }
}

pub(super) const fn clockwise(effect: &Effect, turn: Turn) -> Option<bool> {
    match (effect, turn) {
        (Effect::PolarGradient(gradient), Turn::Spin) => Some(gradient.speed < 0),
        (Effect::PolarGradient(gradient), Turn::Twist) => Some(gradient.twist < 0),
        _ => None,
    }
}

pub(super) fn set_clockwise(effect: &mut Effect, turn: Turn, clockwise: bool) {
    let Effect::PolarGradient(gradient) = effect else {
        return;
    };
    let value = match turn {
        Turn::Spin => &mut gradient.speed,
        Turn::Twist => &mut gradient.twist,
    };
    let magnitude = value.unsigned_abs().clamp(1, i8::MAX.unsigned_abs());
    let magnitude = i8::try_from(magnitude).unwrap_or(i8::MAX);
    *value = if clockwise { -magnitude } else { magnitude };
}

pub(crate) fn level_of(percent: f32) -> u8 {
    byte(percent * LEVEL_MAX / PERCENT_MAX, 0, u8::MAX)
}

pub(crate) fn percent_of(level: u8) -> f32 {
    (f32::from(level) * PERCENT_MAX / LEVEL_MAX).round()
}

impl Dial {
    pub(super) const ALL: [Self; 12] = [
        Self::Period,
        Self::Width,
        Self::ScanSpeed,
        Self::BreathSpeed,
        Self::SpreadSpeed,
        Self::FadeSpeed,
        Self::TrailSpeed,
        Self::Level,
        Self::NoiseSpeed,
        Self::NoiseScale,
        Self::Spin,
        Self::Twist,
    ];

    pub(super) fn bounds(self, capabilities: &Capabilities) -> (f32, f32) {
        match self {
            Self::Period | Self::FadeSpeed => (1.0, LEVEL_MAX),
            Self::Width => (
                1.0,
                f32::from(capabilities.key_columns.max(capabilities.key_rows).max(1)),
            ),
            Self::ScanSpeed => (1.0, 60.0),
            Self::BreathSpeed | Self::TrailSpeed => (1.0, STEP_MS_MAX),
            Self::SpreadSpeed => (1.0, 50.0),
            Self::Level | Self::NoiseSpeed | Self::NoiseScale => (0.0, LEVEL_MAX),
            Self::Spin | Self::Twist => (0.0, SIGNED_MAX),
        }
    }

    pub(super) const fn of(effect: &Effect) -> &'static [Self] {
        match effect {
            Effect::RainbowWave(_) | Effect::HueCycle(_) => &[Self::Period],
            Effect::Scanner(_) => &[Self::Width, Self::ScanSpeed, Self::Level],
            Effect::Breathing(_) => &[Self::BreathSpeed, Self::Level],
            Effect::KeyRipple(_) => &[Self::SpreadSpeed, Self::FadeSpeed, Self::Level],
            Effect::ReactiveFade(_) => &[Self::TrailSpeed, Self::Level],
            Effect::LevelMeter(_)
            | Effect::AnalogSpotlight(_)
            | Effect::AnalogGlow(_)
            | Effect::KeyTest(_) => &[Self::Level],
            Effect::AnalogRgbFill(_) => &[],
            Effect::SimplexNoise(_) => &[Self::NoiseSpeed, Self::NoiseScale],
            Effect::PolarGradient(_) => &[Self::Spin, Self::Twist],
        }
    }

    pub(super) fn raw(self, effect: &Effect) -> Option<f32> {
        let raw = match (self, effect) {
            (
                Self::Period,
                Effect::RainbowWave(RainbowWave { period, .. })
                | Effect::HueCycle(HueCycle { period, .. }),
            ) => *period,
            (Self::Width, Effect::Scanner(scanner)) => scanner.width,
            (Self::ScanSpeed, Effect::Scanner(scanner)) => scanner.steps_per_second,
            (Self::BreathSpeed, Effect::Breathing(breathing)) => breathing.step_ms,
            (Self::SpreadSpeed, Effect::KeyRipple(ripple)) => ripple.steps_per_second,
            (Self::FadeSpeed, Effect::KeyRipple(ripple)) => ripple.decay_step_ms,
            (Self::TrailSpeed, Effect::ReactiveFade(fade)) => fade.step_ms,
            (Self::Level, _) => level(effect)?,
            (Self::NoiseSpeed, Effect::SimplexNoise(noise)) => noise.speed,
            (Self::NoiseScale, Effect::SimplexNoise(noise)) => noise.scale,
            (Self::Spin, Effect::PolarGradient(gradient)) => gradient.speed.unsigned_abs(),
            (Self::Twist, Effect::PolarGradient(gradient)) => gradient.twist.unsigned_abs(),
            _ => return None,
        };
        Some(f32::from(raw))
    }

    pub(super) fn apply(self, effect: &mut Effect, raw: f32) {
        match (self, effect) {
            (
                Self::Period,
                Effect::RainbowWave(RainbowWave { period, .. })
                | Effect::HueCycle(HueCycle { period, .. }),
            ) => *period = byte(raw, 1, u8::MAX),
            (Self::Width, Effect::Scanner(scanner)) => scanner.width = byte(raw, 1, u8::MAX),
            (Self::ScanSpeed, Effect::Scanner(scanner)) => {
                scanner.steps_per_second = byte(raw, 1, 60);
            }
            (Self::BreathSpeed, Effect::Breathing(breathing)) => {
                breathing.step_ms = byte(raw, 1, 40);
            }
            (Self::SpreadSpeed, Effect::KeyRipple(ripple)) => {
                ripple.steps_per_second = byte(raw, 1, 50);
            }
            (Self::FadeSpeed, Effect::KeyRipple(ripple)) => {
                ripple.decay_step_ms = byte(raw, 1, u8::MAX);
            }
            (Self::TrailSpeed, Effect::ReactiveFade(fade)) => fade.step_ms = byte(raw, 1, 40),
            (Self::Level, effect) => set_level(effect, byte(raw, 0, u8::MAX)),
            (Self::NoiseSpeed, Effect::SimplexNoise(noise)) => noise.speed = byte(raw, 0, u8::MAX),
            (Self::NoiseScale, Effect::SimplexNoise(noise)) => noise.scale = byte(raw, 0, u8::MAX),
            (Self::Spin, Effect::PolarGradient(gradient)) => {
                gradient.speed = signed(raw, gradient.speed < 0);
            }
            (Self::Twist, Effect::PolarGradient(gradient)) => {
                gradient.twist = signed(raw, gradient.twist < 0);
            }
            _ => {}
        }
    }

    pub(super) const fn label(self, effect: &Effect) -> &'static str {
        match (self, effect) {
            (
                Self::Period
                | Self::ScanSpeed
                | Self::BreathSpeed
                | Self::TrailSpeed
                | Self::NoiseSpeed,
                _,
            ) => "Speed",
            (Self::Width, _) => "Width",
            (Self::SpreadSpeed, _) => "Spread Speed",
            (Self::FadeSpeed, _) => "Fade Speed",
            (Self::Level, Effect::Breathing(_)) => "Secondary Brightness",
            (Self::Level, _) => "Base Brightness",
            (Self::NoiseScale, _) => "Scale",
            (Self::Spin, _) => "Spin Speed",
            (Self::Twist, _) => "Spiral Speed",
        }
    }

    pub(super) const fn unit(self) -> &'static str {
        match self {
            Self::Period | Self::BreathSpeed | Self::FadeSpeed | Self::TrailSpeed => "s",
            Self::Width => "keys",
            Self::ScanSpeed | Self::SpreadSpeed => "keys/s",
            Self::Level | Self::NoiseSpeed | Self::NoiseScale | Self::Spin | Self::Twist => "%",
        }
    }

    pub(super) fn text(self, raw: f32) -> String {
        let shown = raw * self.factor();
        match self {
            Self::Period | Self::BreathSpeed | Self::FadeSpeed | Self::TrailSpeed => {
                format!("{shown:.2}")
            }
            Self::Width
            | Self::ScanSpeed
            | Self::SpreadSpeed
            | Self::Level
            | Self::NoiseSpeed
            | Self::NoiseScale
            | Self::Spin
            | Self::Twist => format!("{shown:.0}"),
        }
    }

    pub(super) fn raw_of(self, shown: f32, capabilities: &Capabilities) -> f32 {
        let (min, max) = self.bounds(capabilities);
        (shown / self.factor()).round().clamp(min, max)
    }

    const fn factor(self) -> f32 {
        match self {
            Self::Period => PERIOD_SECONDS,
            Self::BreathSpeed => BREATH_STEP_SECONDS,
            Self::FadeSpeed => FADE_STEP_SECONDS,
            Self::TrailSpeed => TRAIL_STEP_SECONDS,
            Self::Width | Self::ScanSpeed | Self::SpreadSpeed => 1.0,
            Self::Level | Self::NoiseSpeed | Self::NoiseScale => PERCENT_MAX / LEVEL_MAX,
            Self::Spin | Self::Twist => PERCENT_MAX / SIGNED_MAX,
        }
    }
}

const fn level(effect: &Effect) -> Option<u8> {
    match effect {
        Effect::Scanner(scanner) => Some(scanner.outside_level),
        Effect::Breathing(breathing) => Some(breathing.min_level),
        Effect::KeyRipple(KeyRipple { base_level, .. })
        | Effect::ReactiveFade(ReactiveFade { base_level, .. })
        | Effect::LevelMeter(LevelMeter { base_level, .. })
        | Effect::AnalogSpotlight(AnalogLevel { base_level, .. })
        | Effect::AnalogGlow(AnalogLevel { base_level, .. })
        | Effect::KeyTest(KeyTest { base_level }) => Some(*base_level),
        _ => None,
    }
}

const fn set_level(effect: &mut Effect, value: u8) {
    match effect {
        Effect::Scanner(scanner) => scanner.outside_level = value,
        Effect::Breathing(breathing) => breathing.min_level = value,
        Effect::KeyRipple(KeyRipple { base_level, .. })
        | Effect::ReactiveFade(ReactiveFade { base_level, .. })
        | Effect::LevelMeter(LevelMeter { base_level, .. })
        | Effect::AnalogSpotlight(AnalogLevel { base_level, .. })
        | Effect::AnalogGlow(AnalogLevel { base_level, .. })
        | Effect::KeyTest(KeyTest { base_level }) => *base_level = value,
        _ => {}
    }
}

const fn color(red: u8, green: u8, blue: u8) -> Color {
    Color {
        red,
        green,
        blue,
        alpha: Color::OPAQUE,
    }
}

fn byte(value: f32, min: u8, max: u8) -> u8 {
    let clamped = value.round().clamp(f32::from(min), f32::from(max));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to a byte range"
    )]
    let value = clamped as u8;
    value
}

fn signed(magnitude: f32, negative: bool) -> i8 {
    let magnitude = byte(magnitude, 0, i8::MAX.unsigned_abs());
    let magnitude = i8::try_from(magnitude).unwrap_or(i8::MAX);
    if negative { -magnitude } else { magnitude }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        configuration::{Capabilities, Validate as _},
        heapless::Vec,
    };

    use super::*;

    fn capabilities() -> Capabilities {
        Capabilities {
            key_rows: 6,
            key_columns: 17,
            profiles: 4,
            linked_profiles: 0,
            layouts: Vec::new(),
            lightbar_leds: 10,
            host_signals: true,
            switch_models: 1,
            edition: None,
        }
    }

    fn catalogue() -> impl Iterator<Item = Effect> {
        BASE_KINDS.into_iter().chain(LAYER_KINDS)
    }

    #[test]
    fn every_effect_is_valid_named_once_and_in_one_grid() {
        let capabilities = capabilities();
        for effect in catalogue() {
            assert_eq!(effect.validate(&capabilities), Ok(()), "{}", name(&effect));
            let twins = catalogue()
                .filter(|other| same_kind(&effect, other) || name(other) == name(&effect))
                .count();
            assert_eq!(twins, 1, "{}", name(&effect));
        }
        assert!(BASE_KINDS.iter().all(is_base));
        assert!(!LAYER_KINDS.iter().any(is_base));
    }

    #[test]
    fn every_dial_reads_what_it_writes_within_its_bounds() {
        let capabilities = capabilities();
        for effect in catalogue() {
            for dial in Dial::of(&effect) {
                let raw = dial
                    .raw(&effect)
                    .unwrap_or_else(|| panic!("{dial:?} of {}", name(&effect)));
                let (min, max) = dial.bounds(&capabilities);
                assert!((min..=max).contains(&raw), "{dial:?}");
                for position in [min, max, raw] {
                    let mut changed = effect;
                    dial.apply(&mut changed, position);
                    assert_eq!(dial.raw(&changed), Some(position), "{dial:?} {position}");
                    assert_eq!(changed.validate(&capabilities), Ok(()), "{dial:?}");
                }
            }
        }
    }

    #[test]
    fn typed_values_convert_to_the_firmware_units() {
        let capabilities = capabilities();
        assert_eq!(Dial::Period.text(16.0), "4.00");
        assert!((Dial::Period.raw_of(4.0, &capabilities) - 16.0).abs() < f32::EPSILON);
        assert!((Dial::Period.raw_of(1000.0, &capabilities) - 255.0).abs() < f32::EPSILON);
        assert_eq!(Dial::FadeSpeed.text(30.0), "0.45");
        assert_eq!(Dial::TrailSpeed.text(4.0), "1.00");
        assert_eq!(Dial::Level.text(30.0), "12");
        assert!((Dial::Level.raw_of(100.0, &capabilities) - 255.0).abs() < f32::EPSILON);
        assert_eq!(Dial::Spin.text(50.0), "39");
        assert_eq!(Dial::Twist.text(70.0), "55");
        assert!((Dial::Width.raw_of(40.0, &capabilities) - 17.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a_spiral_keeps_its_turning_direction_when_its_speed_changes() {
        let mut spiral = SPIRAL;
        assert_eq!(clockwise(&spiral, Turn::Spin), Some(false));
        assert_eq!(clockwise(&spiral, Turn::Twist), Some(true));
        Dial::Twist.apply(&mut spiral, 20.0);
        assert!(matches!(
            spiral,
            Effect::PolarGradient(PolarGradient { twist: -20, .. })
        ));
        set_clockwise(&mut spiral, Turn::Spin, true);
        assert!(matches!(
            spiral,
            Effect::PolarGradient(PolarGradient { speed: -50, .. })
        ));
        Dial::Spin.apply(&mut spiral, 0.0);
        set_clockwise(&mut spiral, Turn::Spin, false);
        assert!(matches!(
            spiral,
            Effect::PolarGradient(PolarGradient { speed: 1, .. })
        ));
    }

    #[test]
    fn levels_show_as_percentages() {
        assert_eq!(level_of(0.0), 0);
        assert_eq!(level_of(100.0), 255);
        assert_eq!(level_of(50.0), 128);
        assert!((percent_of(255) - 100.0).abs() < f32::EPSILON);
        assert!((percent_of(30) - 12.0).abs() < f32::EPSILON);
        for percent in 0..=100_u8 {
            let percent = f32::from(percent);
            assert!((percent_of(level_of(percent)) - percent).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn the_trail_colour_is_its_fixed_colour() {
        let mut trail = LAYER_KINDS[7];
        assert_eq!(colors(&trail), (None, None));
        set_first_color(&mut trail, color(1, 2, 3));
        assert_eq!(colors(&trail), (Some(color(1, 2, 3)), None));
        assert_eq!(
            paint_of(match trail {
                Effect::ReactiveFade(fade) => fade.coloring,
                _ => ReactiveColoring::KeyColor,
            }),
            Paint::Fixed
        );
    }
}
