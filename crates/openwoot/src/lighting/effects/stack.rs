use heapless::Vec;

use super::{
    analog_glow, analog_levels::Levels, analog_rgb_fill, analog_spotlight, breathing, hue_cycle,
    key_ripple, key_test, level_meter, polar_gradient, rainbow_wave, reactive_fade, scanner,
    simplex_noise,
};
use crate::{
    keys::Key,
    lighting::{
        canvas::Canvas,
        config::{Effect, MAX_EFFECTS},
        input::{KeyEvent, KeyTravel},
        leds::{Geometry, Variant},
    },
};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct EffectStack<const ROWS: usize, const COLUMNS: usize> {
    layers: Vec<Layer<ROWS, COLUMNS>, MAX_EFFECTS>,
}

#[derive(Clone, Debug, PartialEq)]
enum Layer<const ROWS: usize, const COLUMNS: usize> {
    RainbowWave(rainbow_wave::State),
    Scanner(scanner::State),
    Breathing(breathing::State),
    HueCycle(hue_cycle::State),
    KeyRipple(key_ripple::State<ROWS, COLUMNS>),
    ReactiveFade(reactive_fade::State<ROWS, COLUMNS>),
    LevelMeter(level_meter::State),
    AnalogSpotlight(Levels<ROWS, COLUMNS>),
    AnalogGlow(Levels<ROWS, COLUMNS>),
    AnalogRgbFill(analog_rgb_fill::State),
    KeyTest(key_test::State<ROWS, COLUMNS>),
    SimplexNoise(simplex_noise::State),
    PolarGradient(polar_gradient::State<ROWS, COLUMNS>),
}

impl<const ROWS: usize, const COLUMNS: usize> EffectStack<ROWS, COLUMNS> {
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
            Effect::AnalogSpotlight(config) => Self::AnalogSpotlight(Levels::new(config)),
            Effect::AnalogGlow(config) => Self::AnalogGlow(Levels::new(config)),
            Effect::AnalogRgbFill(config) => {
                Self::AnalogRgbFill(analog_rgb_fill::State::new(config))
            }
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
            Self::AnalogSpotlight(state) => analog_spotlight::render(state, canvas),
            Self::AnalogGlow(state) => analog_glow::render(state, canvas),
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
