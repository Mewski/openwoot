mod analog;
mod breathing;
mod hue;
mod hue_cycle;
mod key_ripple;
mod key_test;
mod level_meter;
mod polar_gradient;
mod rainbow_wave;
mod reactive_fade;
mod scanner;
mod simplex_noise;

use heapless::Vec;

pub(crate) use super::input::TRAVEL_FULL;
use super::{
    config::{Effect, MAX_EFFECTS},
    frame::Canvas,
    input::{KeyEvent, KeyTravel},
    leds::{Geometry, Key, Variant},
};

pub(crate) type Cells<const ROWS: usize, const COLUMNS: usize> = [[u8; COLUMNS]; ROWS];

const LEVEL_DEAD_ZONE: u8 = 5;
const METER_DEAD_ZONE: u16 = 80;

#[derive(Clone, Debug, PartialEq)]
enum Layer<const ROWS: usize, const COLUMNS: usize> {
    RainbowWave(rainbow_wave::State),
    Scanner(scanner::State),
    Breathing(breathing::State),
    HueCycle(hue_cycle::State),
    KeyRipple(key_ripple::State<ROWS, COLUMNS>),
    ReactiveFade(reactive_fade::State<ROWS, COLUMNS>),
    LevelMeter(level_meter::State),
    AnalogSpotlight(analog::Levels<ROWS, COLUMNS>),
    AnalogGlow(analog::Levels<ROWS, COLUMNS>),
    AnalogRgbFill(analog::Fill),
    KeyTest(key_test::State<ROWS, COLUMNS>),
    SimplexNoise(simplex_noise::State),
    PolarGradient(polar_gradient::State<ROWS, COLUMNS>),
}

impl<const ROWS: usize, const COLUMNS: usize> Layer<ROWS, COLUMNS> {
    fn new(effect: Effect) -> Self {
        match effect {
            Effect::RainbowWave(config) => Self::RainbowWave(rainbow_wave::State::new(config)),
            Effect::Scanner(config) => Self::Scanner(scanner::State::new(config)),
            Effect::Breathing(config) => Self::Breathing(breathing::State::new(config)),
            Effect::HueCycle(config) => Self::HueCycle(hue_cycle::State::new(config)),
            Effect::KeyRipple(config) => Self::KeyRipple(key_ripple::State::new(config)),
            Effect::ReactiveFade(config) => Self::ReactiveFade(reactive_fade::State::new(config)),
            Effect::LevelMeter(config) => Self::LevelMeter(level_meter::State::new(config)),
            Effect::AnalogSpotlight(config) => Self::AnalogSpotlight(analog::Levels::new(config)),
            Effect::AnalogGlow(config) => Self::AnalogGlow(analog::Levels::new(config)),
            Effect::AnalogRgbFill(config) => Self::AnalogRgbFill(analog::Fill::new(config)),
            Effect::KeyTest(config) => Self::KeyTest(key_test::State::new(config)),
            Effect::SimplexNoise(config) => Self::SimplexNoise(simplex_noise::State::new(config)),
            Effect::PolarGradient(config) => {
                Self::PolarGradient(polar_gradient::State::new(config))
            }
        }
    }

    fn update(&mut self, effect: Effect) -> bool {
        match (self, effect) {
            (Self::RainbowWave(state), Effect::RainbowWave(config)) => state.update(config),
            (Self::Scanner(state), Effect::Scanner(config)) => {
                state.update::<ROWS, COLUMNS>(config);
            }
            (Self::Breathing(state), Effect::Breathing(config)) => state.update(config),
            (Self::HueCycle(state), Effect::HueCycle(config)) => state.update(config),
            (Self::KeyRipple(state), Effect::KeyRipple(config)) => state.update(config),
            (Self::ReactiveFade(state), Effect::ReactiveFade(config)) => state.update(config),
            (Self::LevelMeter(state), Effect::LevelMeter(config)) => state.update(config),
            (Self::AnalogSpotlight(state), Effect::AnalogSpotlight(config))
            | (Self::AnalogGlow(state), Effect::AnalogGlow(config)) => state.update(config),
            (Self::AnalogRgbFill(state), Effect::AnalogRgbFill(config)) => state.update(config),
            (Self::KeyTest(state), Effect::KeyTest(config)) => state.update(config),
            (Self::SimplexNoise(state), Effect::SimplexNoise(config)) => state.update(config),
            (Self::PolarGradient(state), Effect::PolarGradient(config)) => state.update(config),
            _ => return false,
        }
        true
    }

    const fn needs_render(&self) -> bool {
        match self {
            Self::KeyRipple(state) => state.needs_render(),
            Self::ReactiveFade(state) => state.needs_render(),
            Self::LevelMeter(state) => state.needs_render(),
            Self::AnalogSpotlight(state) | Self::AnalogGlow(state) => state.needs_render(),
            Self::KeyTest(state) => state.needs_render(),
            Self::RainbowWave(_)
            | Self::Scanner(_)
            | Self::Breathing(_)
            | Self::HueCycle(_)
            | Self::AnalogRgbFill(_)
            | Self::SimplexNoise(_)
            | Self::PolarGradient(_) => true,
        }
    }

    fn render(&mut self, canvas: &mut impl Canvas, geometry: Geometry, elapsed_ms: u32) {
        match self {
            Self::RainbowWave(state) => state.render::<ROWS, COLUMNS>(canvas, elapsed_ms),
            Self::Scanner(state) => state.render::<ROWS, COLUMNS>(
                canvas,
                usize::from(geometry.first_key_row),
                elapsed_ms,
            ),
            Self::Breathing(state) => state.render::<ROWS, COLUMNS>(canvas, elapsed_ms),
            Self::HueCycle(state) => state.render::<ROWS, COLUMNS>(canvas, elapsed_ms),
            Self::KeyRipple(state) => state.render(canvas, elapsed_ms),
            Self::ReactiveFade(state) => state.render(canvas, elapsed_ms),
            Self::LevelMeter(state) => state.render(canvas, geometry.level_meter_segments),
            Self::AnalogSpotlight(state) => state.render_spotlight(canvas),
            Self::AnalogGlow(state) => state.render_glow(canvas),
            Self::AnalogRgbFill(state) => state.render::<ROWS, COLUMNS>(canvas),
            Self::KeyTest(state) => state.render(canvas),
            Self::SimplexNoise(state) => state.render::<ROWS, COLUMNS>(canvas, elapsed_ms),
            Self::PolarGradient(state) => state.render(canvas, elapsed_ms),
        }
    }

    const fn consumes_travel(&self) -> bool {
        matches!(
            self,
            Self::LevelMeter(_)
                | Self::AnalogSpotlight(_)
                | Self::AnalogGlow(_)
                | Self::AnalogRgbFill(_)
        )
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Layers<const ROWS: usize, const COLUMNS: usize> {
    layers: Vec<Layer<ROWS, COLUMNS>, MAX_EFFECTS>,
}

impl<const ROWS: usize, const COLUMNS: usize> Layers<ROWS, COLUMNS> {
    #[must_use]
    pub const fn new() -> Self {
        Self { layers: Vec::new() }
    }

    pub fn apply(&mut self, effects: &[Effect]) {
        for (index, &effect) in effects.iter().take(MAX_EFFECTS).enumerate() {
            match self.layers.get_mut(index) {
                Some(layer) => {
                    if !layer.update(effect) {
                        *layer = Layer::new(effect);
                    }
                }
                None => {
                    if self.layers.push(Layer::new(effect)).is_err() {
                        break;
                    }
                }
            }
        }
        self.layers.truncate(effects.len().min(MAX_EFFECTS));
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    #[must_use]
    pub fn needs_render(&self) -> bool {
        self.layers.iter().any(Layer::needs_render)
    }

    #[must_use]
    pub fn consumes_travel(&self) -> bool {
        self.layers.iter().any(Layer::consumes_travel)
    }

    pub fn reset(&mut self, effects: &[Effect]) {
        self.layers.clear();
        self.apply(effects);
    }

    pub fn render(&mut self, canvas: &mut impl Canvas, geometry: Geometry, elapsed_ms: u32) {
        for layer in &mut self.layers {
            layer.render(canvas, geometry, elapsed_ms);
        }
    }

    pub fn event(
        &mut self,
        key: Key,
        event: KeyEvent,
        now_ms: u32,
        variant: &Variant<ROWS, COLUMNS>,
    ) {
        for layer in &mut self.layers {
            match layer {
                Layer::ReactiveFade(state) if event == KeyEvent::Pressed => {
                    state.press(key, now_ms, variant);
                }
                Layer::KeyRipple(state) if event == KeyEvent::Pressed => state.press(key, now_ms),
                Layer::KeyTest(state) => state.event(key, event),
                _ => {}
            }
        }
    }

    pub fn feed(&mut self, key: Key, travel: u16, input: &impl KeyTravel) {
        for layer in &mut self.layers {
            match layer {
                Layer::LevelMeter(state) => state.feed(key, travel, input),
                Layer::AnalogSpotlight(state) | Layer::AnalogGlow(state) => {
                    state.feed(key, travel, input);
                }
                Layer::AnalogRgbFill(state) => state.feed::<ROWS, COLUMNS>(key, travel, input),
                _ => {}
            }
        }
    }

    pub fn release_all(&mut self) {
        for layer in &mut self.layers {
            match layer {
                Layer::LevelMeter(state) => state.release_all(),
                Layer::AnalogSpotlight(state) => state.release_all(false),
                Layer::AnalogGlow(state) => state.release_all(true),
                Layer::AnalogRgbFill(state) => state.release_all(),
                _ => {}
            }
        }
    }
}

pub(crate) fn pick_press_color(last_color: &mut u8, now_ms: u32) -> u8 {
    let tick = low_8(now_ms & 7);
    let color = if tick + 1 > 6 {
        if *last_color == 7 { 1 } else { 7 }
    } else if *last_color == tick + 1 {
        if *last_color == 7 { 1 } else { tick + 2 }
    } else {
        tick + 1
    };
    *last_color = color;
    color & 0x07
}

pub(crate) fn travel_level_8(gamepad_curve: bool, travel: u16, input: &impl KeyTravel) -> u8 {
    if !gamepad_curve || !input.gamepad_mode() {
        let level = analog::linear_level(travel);
        return if level > LEVEL_DEAD_ZONE { level } else { 0 };
    }
    if travel == 0 {
        return 0;
    }
    float_to_byte(input.response_curve(travel) * 255.0)
}

pub(crate) fn travel_level_12(gamepad_curve: bool, travel: u16, input: &impl KeyTravel) -> u16 {
    if !gamepad_curve || !input.gamepad_mode() {
        return if travel > METER_DEAD_ZONE { travel } else { 0 };
    }
    if travel == 0 {
        return 0;
    }
    let curve = input.response_curve(travel);
    if curve.is_nan() || curve <= 0.0 {
        0
    } else if curve >= 1.0 {
        TRAVEL_FULL
    } else {
        low_16(float_to_u32(curve * 4095.0))
    }
}

pub(crate) const fn low_8(value: u32) -> u8 {
    value.to_le_bytes()[0]
}

pub(crate) const fn low_16(value: u32) -> u16 {
    let [low, high, _, _] = value.to_le_bytes();
    u16::from_le_bytes([low, high])
}

fn float_to_byte(value: f32) -> u8 {
    low_8(float_to_u32(value))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "levels are converted to integers by truncation, negative values give 0"
)]
fn float_to_u32(value: f32) -> u32 {
    value as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn press_colours_never_repeat_the_previous_one() {
        let mut last = 0;
        assert_eq!(pick_press_color(&mut last, 0), 1);
        assert_eq!(pick_press_color(&mut last, 8), 2);
        assert_eq!(pick_press_color(&mut last, 6), 7);
        assert_eq!(pick_press_color(&mut last, 7), 1);
        assert_eq!(pick_press_color(&mut last, 0), 2);
        assert_eq!(pick_press_color(&mut last, 5), 6);
        assert_eq!(pick_press_color(&mut last, 5), 7);
        for now in 0..64 {
            let previous = last;
            let color = pick_press_color(&mut last, now);
            assert!((1..=7).contains(&color));
            assert_ne!(color, previous);
        }
    }

    #[test]
    fn levels_ignore_the_dead_zones_without_the_gamepad_curve() {
        assert_eq!(travel_level_8(false, 0, &()), 0);
        assert_eq!(travel_level_8(false, 96, &()), 0);
        assert_eq!(travel_level_8(false, 112, &()), 6);
        assert_eq!(travel_level_8(true, 4095, &()), 255);
        assert_eq!(travel_level_12(false, 80, &()), 0);
        assert_eq!(travel_level_12(false, 81, &()), 81);
    }

    struct Gamepad;

    impl KeyTravel for Gamepad {
        fn gamepad_mode(&self) -> bool {
            true
        }

        fn response_curve(&self, travel: u16) -> f32 {
            if travel < 400 { 0.0 } else { 0.5 }
        }
    }

    #[test]
    fn levels_follow_the_gamepad_curve_in_gamepad_mode() {
        assert_eq!(travel_level_8(true, 300, &Gamepad), 0);
        assert_eq!(travel_level_8(true, 1000, &Gamepad), 127);
        assert_eq!(travel_level_8(false, 1000, &Gamepad), 62);
        assert_eq!(travel_level_12(true, 300, &Gamepad), 0);
        assert_eq!(travel_level_12(true, 1000, &Gamepad), 2047);
    }
}
