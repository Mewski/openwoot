use std::mem;

use openwoot_protocol::lighting::{
    AnalogLevel, AnalogRgbFill, Breathing, Color, Effect, HueCycle, HueRange, KeyRipple, KeyTest,
    LevelMeter, PolarGradient, RainbowWave, ReactiveColoring, ReactiveFade, Scanner,
    ScannerDirection, SimplexNoise,
};

pub(super) const CATALOGUE: [Effect; 13] = [
    Effect::RainbowWave(RainbowWave {
        hues: HueRange::FullCircle,
        period: 16,
        forward: false,
    }),
    Effect::Scanner(Scanner {
        width: 1,
        steps_per_second: 15,
        outside_level: 30,
        direction: ScannerDirection::Left,
    }),
    Effect::Breathing(Breathing {
        step_ms: 15,
        min_level: 0,
    }),
    Effect::HueCycle(HueCycle {
        hues: HueRange::FullCircle,
        period: 48,
    }),
    Effect::KeyRipple(KeyRipple {
        palette: false,
        inverted: false,
        steps_per_second: 20,
        decay_step_ms: 30,
        base_level: 30,
    }),
    Effect::ReactiveFade(ReactiveFade {
        base_level: 30,
        step_ms: 4,
        coloring: ReactiveColoring::KeyColor,
        inverted: false,
    }),
    Effect::LevelMeter(LevelMeter {
        base_level: 30,
        gamepad_curve: false,
    }),
    Effect::AnalogSpotlight(AnalogLevel {
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
    Effect::KeyTest(KeyTest { base_level: 140 }),
    Effect::SimplexNoise(SimplexNoise {
        from: color(30, 0, 255),
        to: color(0, 255, 10),
        speed: 50,
        scale: 25,
    }),
    Effect::PolarGradient(PolarGradient {
        from: color(255, 0, 255),
        to: color(0, 255, 255),
        speed: 50,
        twist: -70,
    }),
];

pub(super) const HUE_RANGES: [(HueRange, &str); 6] = [
    (HueRange::FullCircle, "Rainbow"),
    (HueRange::From110To184, "Ocean"),
    (HueRange::From2To39, "Inferno"),
    (HueRange::From64To114, "Spring"),
    (HueRange::From214To254, "Pink!"),
    (HueRange::From113To204, "Blue Spectrum"),
];

pub(super) const DIRECTIONS: [(ScannerDirection, &str); 4] = [
    (ScannerDirection::Left, "Left"),
    (ScannerDirection::Right, "Right"),
    (ScannerDirection::Up, "Up"),
    (ScannerDirection::Down, "Down"),
];

const MS_PER_PERIOD: f32 = 250.0;
const STEPS_PER_BREATH: f32 = 252.0;
const LEVELS_PER_FADE: f32 = 250.0;
const LEVELS_PER_RIPPLE_FADE: f32 = 15.0;
const MS_PER_SECOND: f32 = 1000.0;
const LEVEL_MAX: f32 = 255.0;
const PERCENT_MAX: f32 = 100.0;
const STEP_MS_MAX: u8 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Dial {
    Cycle,
    Steps,
    Spread,
    Level,
    Breath,
    Fade,
    Decay,
    Drift,
    Scale,
    Spin,
    Twist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Paint {
    KeyColor,
    Palette,
    Fixed,
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
        Effect::RainbowWave(_) => "A band of colours moves across the keys.",
        Effect::Scanner(_) => {
            "A window sweeps back and forth over the keys; the keys outside it are dimmed."
        }
        Effect::Breathing(_) => "All keys fade down and up again.",
        Effect::HueCycle(_) => "All keys change colour together.",
        Effect::KeyRipple(_) => "Each key press sends ripples across the keys.",
        Effect::ReactiveFade(_) => "Pressed keys light up and fade out.",
        Effect::LevelMeter(_) => {
            "The number row shows how deep the deepest key is pressed, as a bar."
        }
        Effect::AnalogSpotlight(_) => {
            "A square of keys around a pressed key lights up; it grows with the key's depth."
        }
        Effect::AnalogGlow(_) => "Pressed keys and their neighbours glow with the key's depth.",
        Effect::AnalogRgbFill(_) => {
            "The three rightmost keys of the bottom row mix red, green and blue by their depth \
             into the colour of every key."
        }
        Effect::KeyTest(_) => {
            "Keys show green while they send a press and red while they are below the actuation \
             point without sending, so rapid trigger can be watched."
        }
        Effect::SimplexNoise(_) => "Two colours drift across the keys like clouds.",
        Effect::PolarGradient(_) => "Two colours swirl around the centre of the keyboard.",
    }
}

pub(super) fn same_kind(effect: &Effect, other: &Effect) -> bool {
    mem::discriminant(effect) == mem::discriminant(other)
}

pub(super) fn summary(effect: &Effect) -> String {
    let hues = hue_range(effect).map(hue_name);
    let reading = Dial::of(effect)
        .first()
        .and_then(|dial| dial.reading(effect));
    match (hues, reading) {
        (Some(hues), Some(reading)) => format!("{hues} · {reading}"),
        (Some(text), None) | (None, Some(text)) => text,
        (None, None) => String::new(),
    }
}

pub(super) const fn hue_range(effect: &Effect) -> Option<HueRange> {
    match effect {
        Effect::RainbowWave(wave) => Some(wave.hues),
        Effect::HueCycle(cycle) => Some(cycle.hues),
        _ => None,
    }
}

pub(super) fn hue_name(hues: HueRange) -> String {
    HUE_RANGES
        .iter()
        .find(|(range, _)| *range == hues)
        .map_or_else(String::new, |(_, label)| (*label).to_owned())
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

pub(crate) fn level_of(percent: f32) -> u8 {
    byte(percent * LEVEL_MAX / PERCENT_MAX, 0, u8::MAX)
}

pub(crate) fn percent_of(level: u8) -> f32 {
    (f32::from(level) * PERCENT_MAX / LEVEL_MAX).round()
}

impl Dial {
    pub(super) const ALL: [Self; 11] = [
        Self::Cycle,
        Self::Steps,
        Self::Spread,
        Self::Level,
        Self::Breath,
        Self::Fade,
        Self::Decay,
        Self::Drift,
        Self::Scale,
        Self::Spin,
        Self::Twist,
    ];

    pub(super) const fn bounds(self) -> (f32, f32) {
        match self {
            Self::Cycle | Self::Decay => (1.0, 255.0),
            Self::Steps => (1.0, 60.0),
            Self::Spread => (1.0, 50.0),
            Self::Level => (0.0, PERCENT_MAX),
            Self::Breath | Self::Fade => (1.0, 40.0),
            Self::Drift | Self::Scale => (0.0, 255.0),
            Self::Spin | Self::Twist => (-128.0, 127.0),
        }
    }

    pub(super) const fn of(effect: &Effect) -> &'static [Self] {
        match effect {
            Effect::RainbowWave(_) | Effect::HueCycle(_) => &[Self::Cycle],
            Effect::Scanner(_) => &[Self::Steps, Self::Level],
            Effect::Breathing(_) => &[Self::Breath, Self::Level],
            Effect::KeyRipple(_) => &[Self::Spread, Self::Decay, Self::Level],
            Effect::ReactiveFade(_) => &[Self::Fade, Self::Level],
            Effect::LevelMeter(_)
            | Effect::AnalogSpotlight(_)
            | Effect::AnalogGlow(_)
            | Effect::KeyTest(_) => &[Self::Level],
            Effect::AnalogRgbFill(_) => &[],
            Effect::SimplexNoise(_) => &[Self::Drift, Self::Scale],
            Effect::PolarGradient(_) => &[Self::Spin, Self::Twist],
        }
    }

    pub(super) fn position(self, effect: &Effect) -> Option<f32> {
        match (self, effect) {
            (
                Self::Cycle,
                Effect::RainbowWave(RainbowWave { period, .. })
                | Effect::HueCycle(HueCycle { period, .. }),
            ) => Some(f32::from(u8::MAX - *period) + 1.0),
            (Self::Steps, Effect::Scanner(scanner)) => Some(f32::from(scanner.steps_per_second)),
            (Self::Spread, Effect::KeyRipple(ripple)) => Some(f32::from(ripple.steps_per_second)),
            (Self::Level, _) => level(effect).map(percent_of),
            (Self::Breath, Effect::Breathing(Breathing { step_ms, .. }))
            | (Self::Fade, Effect::ReactiveFade(ReactiveFade { step_ms, .. })) => {
                Some(f32::from(STEP_MS_MAX.saturating_sub(*step_ms)) + 1.0)
            }
            (Self::Decay, Effect::KeyRipple(ripple)) => {
                Some(f32::from(u8::MAX - ripple.decay_step_ms) + 1.0)
            }
            (Self::Drift, Effect::SimplexNoise(noise)) => Some(f32::from(noise.speed)),
            (Self::Scale, Effect::SimplexNoise(noise)) => Some(f32::from(noise.scale)),
            (Self::Spin, Effect::PolarGradient(gradient)) => Some(f32::from(gradient.speed)),
            (Self::Twist, Effect::PolarGradient(gradient)) => Some(f32::from(gradient.twist)),
            _ => None,
        }
    }

    pub(super) fn apply(self, effect: &mut Effect, position: f32) {
        let (min, max) = self.bounds();
        let position = position.clamp(min, max);
        match (self, effect) {
            (
                Self::Cycle,
                Effect::RainbowWave(RainbowWave { period, .. })
                | Effect::HueCycle(HueCycle { period, .. }),
            ) => {
                *period = u8::MAX - byte(position, 1, u8::MAX) + 1;
            }
            (Self::Steps, Effect::Scanner(scanner)) => {
                scanner.steps_per_second = byte(position, 1, 60);
            }
            (Self::Spread, Effect::KeyRipple(ripple)) => {
                ripple.steps_per_second = byte(position, 1, 50);
            }
            (Self::Level, effect) => set_level(effect, level_of(position)),
            (Self::Breath, Effect::Breathing(Breathing { step_ms, .. }))
            | (Self::Fade, Effect::ReactiveFade(ReactiveFade { step_ms, .. })) => {
                *step_ms = STEP_MS_MAX - byte(position, 1, STEP_MS_MAX) + 1;
            }
            (Self::Decay, Effect::KeyRipple(ripple)) => {
                ripple.decay_step_ms = u8::MAX - byte(position, 1, u8::MAX) + 1;
            }
            (Self::Drift, Effect::SimplexNoise(noise)) => noise.speed = byte(position, 0, u8::MAX),
            (Self::Scale, Effect::SimplexNoise(noise)) => noise.scale = byte(position, 0, u8::MAX),
            (Self::Spin, Effect::PolarGradient(gradient)) => gradient.speed = signed(position),
            (Self::Twist, Effect::PolarGradient(gradient)) => gradient.twist = signed(position),
            _ => {}
        }
    }

    pub(super) const fn label(self, effect: &Effect) -> &'static str {
        match (self, effect) {
            (Self::Cycle | Self::Steps | Self::Breath | Self::Drift, _) => "Speed",
            (Self::Spread, _) => "Spread speed",
            (Self::Level, Effect::Breathing(_)) => "Minimum brightness",
            (Self::Level, _) => "Base brightness",
            (Self::Fade | Self::Decay, _) => "Fade speed",
            (Self::Scale, _) => "Scale",
            (Self::Spin, _) => "Spin speed",
            (Self::Twist, _) => "Spiral",
        }
    }

    pub(super) const fn detail(self, effect: &Effect) -> Option<&'static str> {
        match (self, effect) {
            (Self::Level, Effect::Scanner(_)) => Some("Brightness of the keys outside the window."),
            (Self::Level, Effect::Breathing(_)) => Some("Brightness at the bottom of each breath."),
            (Self::Level, Effect::KeyRipple(_)) => {
                Some("Brightness of the keys a ripple has not reached.")
            }
            (Self::Level, Effect::ReactiveFade(_)) => {
                Some("Brightness of the keys that are not lit by a press.")
            }
            (Self::Level, Effect::LevelMeter(_)) => {
                Some("Brightness of the number row keys the bar does not reach.")
            }
            (Self::Level, Effect::AnalogSpotlight(_) | Effect::AnalogGlow(_)) => {
                Some("Brightness of the keys away from pressed keys.")
            }
            (Self::Level, Effect::KeyTest(_)) => Some("Brightness of the keys at rest."),
            (Self::Spin | Self::Twist, _) => Some("Negative values turn the other way."),
            _ => None,
        }
    }

    pub(super) fn reading(self, effect: &Effect) -> Option<String> {
        let reading = match (self, effect) {
            (
                Self::Cycle,
                Effect::RainbowWave(RainbowWave { period, .. })
                | Effect::HueCycle(HueCycle { period, .. }),
            ) => {
                let seconds = f32::from(*period) * MS_PER_PERIOD / MS_PER_SECOND;
                format!("{seconds:.2} s per cycle")
            }
            (Self::Steps, Effect::Scanner(scanner)) => {
                format!("{} steps per second", scanner.steps_per_second)
            }
            (Self::Spread, Effect::KeyRipple(ripple)) => {
                format!("{} keys per second", ripple.steps_per_second)
            }
            (Self::Level, _) => format!("{} %", percent_of(level(effect)?)),
            (Self::Breath, Effect::Breathing(breathing)) => {
                let seconds = f32::from(breathing.step_ms) * STEPS_PER_BREATH / MS_PER_SECOND;
                format!("{seconds:.1} s per breath")
            }
            (Self::Fade, Effect::ReactiveFade(fade)) => {
                let seconds = f32::from(fade.step_ms) * LEVELS_PER_FADE / MS_PER_SECOND;
                format!("fades in about {seconds:.2} s")
            }
            (Self::Decay, Effect::KeyRipple(ripple)) => {
                let seconds =
                    f32::from(ripple.decay_step_ms) * LEVELS_PER_RIPPLE_FADE / MS_PER_SECOND;
                format!("fades in about {seconds:.2} s")
            }
            (Self::Drift, Effect::SimplexNoise(noise)) => noise.speed.to_string(),
            (Self::Scale, Effect::SimplexNoise(noise)) => noise.scale.to_string(),
            (Self::Spin, Effect::PolarGradient(gradient)) => gradient.speed.to_string(),
            (Self::Twist, Effect::PolarGradient(gradient)) => gradient.twist.to_string(),
            _ => return None,
        };
        Some(reading)
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

fn byte(position: f32, min: u8, max: u8) -> u8 {
    let clamped = position.round().clamp(f32::from(min), f32::from(max));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to a byte range"
    )]
    let value = clamped as u8;
    value
}

fn signed(position: f32) -> i8 {
    let clamped = position
        .round()
        .clamp(f32::from(i8::MIN), f32::from(i8::MAX));
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the value is rounded and clamped to the signed byte range"
    )]
    let value = clamped as i8;
    value
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
            layouts: Vec::new(),
            lightbar_leds: 10,
            host_signals: true,
            switch_models: 1,
            edition: None,
        }
    }

    #[test]
    fn every_catalogue_effect_is_valid_and_named_once() {
        let capabilities = capabilities();
        for effect in &CATALOGUE {
            assert_eq!(effect.validate(&capabilities), Ok(()), "{}", name(effect));
            let twins = CATALOGUE
                .iter()
                .filter(|other| same_kind(effect, other) || name(other) == name(effect))
                .count();
            assert_eq!(twins, 1, "{}", name(effect));
        }
    }

    #[test]
    fn every_dial_of_an_effect_reads_and_writes_it() {
        for effect in &CATALOGUE {
            for dial in Dial::of(effect) {
                let position = dial
                    .position(effect)
                    .unwrap_or_else(|| panic!("{dial:?} of {}", name(effect)));
                let mut changed = *effect;
                dial.apply(&mut changed, position);
                assert_eq!(dial.position(&changed), Some(position), "{dial:?}");
                assert!(dial.reading(effect).is_some(), "{dial:?}");
                let (min, max) = dial.bounds();
                assert!((min..=max).contains(&position), "{dial:?}");
            }
        }
    }

    #[test]
    fn dial_extremes_stay_within_the_firmware_ranges() {
        let capabilities = capabilities();
        for effect in &CATALOGUE {
            for dial in Dial::of(effect) {
                let (min, max) = dial.bounds();
                for position in [min - 10.0, min, max, max + 10.0] {
                    let mut changed = *effect;
                    dial.apply(&mut changed, position);
                    assert_eq!(
                        changed.validate(&capabilities),
                        Ok(()),
                        "{dial:?} {position}"
                    );
                }
            }
        }
    }

    #[test]
    fn faster_speeds_shorten_the_cycle() {
        let mut wave = CATALOGUE[0];
        Dial::Cycle.apply(&mut wave, 255.0);
        assert!(matches!(
            wave,
            Effect::RainbowWave(RainbowWave { period: 1, .. })
        ));
        Dial::Cycle.apply(&mut wave, 1.0);
        assert!(matches!(
            wave,
            Effect::RainbowWave(RainbowWave { period: 255, .. })
        ));
        assert_eq!(
            Dial::Cycle.reading(&wave).as_deref(),
            Some("63.75 s per cycle")
        );
        let mut breath = CATALOGUE[2];
        Dial::Breath.apply(&mut breath, 40.0);
        assert!(matches!(
            breath,
            Effect::Breathing(Breathing { step_ms: 1, .. })
        ));
        Dial::Breath.apply(&mut breath, 1.0);
        assert!(matches!(
            breath,
            Effect::Breathing(Breathing { step_ms: 40, .. })
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
        let mut trail = CATALOGUE[5];
        assert_eq!(colors(&trail), (None, None));
        set_first_color(&mut trail, color(1, 2, 3));
        assert_eq!(colors(&trail), (Some(color(1, 2, 3)), None));
        assert!(matches!(
            trail,
            Effect::ReactiveFade(ReactiveFade {
                coloring: ReactiveColoring::Fixed(_),
                ..
            })
        ));
    }

    #[test]
    fn the_summary_names_the_colours_and_the_speed() {
        assert_eq!(summary(&CATALOGUE[0]), "Rainbow · 4.00 s per cycle");
        assert_eq!(summary(&CATALOGUE[1]), "15 steps per second");
        assert_eq!(summary(&CATALOGUE[9]), "");
    }
}
