use openwoot_protocol::{
    gamepad::{self, Control},
    keys::{self, Action, Distance, Override},
    lighting::{self, Effect, HueRange, RainbowWave},
};

const USAGE_W: u8 = 0x1A;
const USAGE_A: u8 = 0x04;
const USAGE_S: u8 = 0x16;
const USAGE_D: u8 = 0x07;
const USAGE_LEFT_GUI: u8 = 0xE3;
const FPS_ACTUATION_MM: f32 = 1.0;
const FPS_RAPID_TRIGGER_MM: f32 = 0.2;
const FPS_MOVEMENT_ACTUATION_MM: f32 = 0.4;
const FPS_WINDOWS_KEY_ACTUATION_MM: f32 = 3.0;
const TYPING_ACTUATION_MM: f32 = 1.5;
const TYPING_WAVE: Effect = Effect::RainbowWave(RainbowWave {
    hues: HueRange::From113To204,
    period: 16,
    forward: false,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Preset {
    Fps,
    AnalogMovement,
    Racing,
    Typing,
    Custom,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Movement {
    up: Option<usize>,
    left: Option<usize>,
    down: Option<usize>,
    right: Option<usize>,
    windows: Option<usize>,
}

impl Preset {
    pub(super) const ALL: [Self; 5] = [
        Self::Fps,
        Self::AnalogMovement,
        Self::Racing,
        Self::Typing,
        Self::Custom,
    ];

    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Fps => "FPS Game Profile",
            Self::AnalogMovement => "Analog Movement Profile",
            Self::Racing => "Racing Profile",
            Self::Typing => "Typing Profile",
            Self::Custom => "Custom Profile",
        }
    }

    pub(super) const fn description(self) -> &'static str {
        match self {
            Self::Fps => "Quick response times and precise control for shooters.",
            Self::AnalogMovement => "Smooth, analog-like movement for nuanced character control.",
            Self::Racing => "Enhanced steering and acceleration control for racing games.",
            Self::Typing => "Comfort and accuracy for typing and everyday use.",
            Self::Custom => "A blank slate for customization.",
        }
    }

    pub(super) const fn changes_keys(self) -> bool {
        matches!(self, Self::Fps | Self::Typing)
    }

    pub(super) const fn changes_gamepad(self) -> bool {
        matches!(self, Self::AnalogMovement | Self::Racing)
    }

    pub(super) const fn changes_lighting(self) -> bool {
        matches!(self, Self::Typing)
    }

    pub(super) fn tune_lighting(self, settings: &mut lighting::Settings) {
        if self != Self::Typing {
            return;
        }
        settings.effects.clear();
        settings.effects.push(TYPING_WAVE).ok();
    }

    pub(super) fn tune_keys(
        self,
        settings: &mut keys::Settings,
        overrides: &mut keys::Overrides,
        movement: Movement,
        distance: impl Fn(usize, f32) -> Distance,
    ) {
        let reference = movement.up.unwrap_or_default();
        match self {
            Self::Fps => {
                settings.actuation = distance(reference, FPS_ACTUATION_MM);
                settings.rapid_trigger_enabled = true;
                settings.rapid_trigger.press = distance(reference, FPS_RAPID_TRIGGER_MM);
                settings.rapid_trigger.release = None;
                for key in [movement.up, movement.left, movement.down, movement.right]
                    .into_iter()
                    .flatten()
                {
                    set_actuation(overrides, key, distance(key, FPS_MOVEMENT_ACTUATION_MM));
                }
                if let Some(key) = movement.windows {
                    set_actuation(overrides, key, distance(key, FPS_WINDOWS_KEY_ACTUATION_MM));
                }
            }
            Self::Typing => {
                settings.actuation = distance(reference, TYPING_ACTUATION_MM);
                settings.rapid_trigger_enabled = false;
                overrides
                    .iter_mut()
                    .for_each(|entry| *entry = Override::default());
            }
            Self::AnalogMovement | Self::Racing | Self::Custom => {}
        }
    }

    pub(super) fn map_gamepad(self, mapping: &mut gamepad::Mapping, movement: Movement) {
        let controls = match self {
            Self::AnalogMovement => [
                Control::LeftStickUp,
                Control::LeftStickLeft,
                Control::LeftStickDown,
                Control::LeftStickRight,
            ],
            Self::Racing => [
                Control::RightTrigger,
                Control::LeftStickLeft,
                Control::LeftTrigger,
                Control::LeftStickRight,
            ],
            Self::Fps | Self::Typing | Self::Custom => return,
        };
        mapping
            .bindings
            .iter_mut()
            .for_each(|binding| *binding = None);
        let keys = [movement.up, movement.left, movement.down, movement.right];
        for (key, control) in keys.into_iter().zip(controls) {
            if let Some(binding) = key.and_then(|key| mapping.bindings.get_mut(key)) {
                *binding = Some(control);
            }
        }
    }
}

impl Movement {
    pub(super) fn find(keymap: &[Action]) -> Self {
        let position = |usage: u8| {
            keymap
                .iter()
                .position(|action| *action == Action::Keyboard(usage))
        };
        Self {
            up: position(USAGE_W),
            left: position(USAGE_A),
            down: position(USAGE_S),
            right: position(USAGE_D),
            windows: position(USAGE_LEFT_GUI),
        }
    }
}

fn set_actuation(overrides: &mut keys::Overrides, key: usize, actuation: Distance) {
    if let Some(entry) = overrides.get_mut(key) {
        *entry = Override {
            actuation: Some(actuation),
            ..*entry
        };
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        heapless,
        keys::{RapidTrigger, RapidTriggerOverride, ScanMode, TapDuration},
    };

    use super::*;

    fn keymap() -> Vec<Action> {
        vec![
            Action::Keyboard(USAGE_LEFT_GUI),
            Action::Keyboard(USAGE_W),
            Action::None,
            Action::Keyboard(USAGE_A),
            Action::Keyboard(USAGE_S),
            Action::Keyboard(USAGE_D),
        ]
    }

    fn hundredths(millimetres: f32) -> Distance {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "test distances are small and positive"
        )]
        let fraction = (millimetres * 100.0).round() as u16;
        Distance::from_fraction(fraction)
    }

    #[test]
    fn movement_finds_the_keys_by_their_binding() {
        assert_eq!(
            Movement::find(&keymap()),
            Movement {
                up: Some(1),
                left: Some(3),
                down: Some(4),
                right: Some(5),
                windows: Some(0),
            }
        );
    }

    #[test]
    fn fps_preset_sets_actuation_and_rapid_trigger() {
        let mut settings = keys::Settings {
            actuation: hundredths(2.0),
            rapid_trigger_enabled: false,
            rapid_trigger: RapidTrigger {
                press: hundredths(0.5),
                release: Some(hundredths(0.5)),
                continuous: true,
            },
            tap_duration: TapDuration::FortyMilliseconds,
            scan_mode: ScanMode::Parallel,
            keyboard_in_gamepad_mode: false,
        };
        let mut overrides: keys::Overrides = heapless::Vec::new();
        for _ in 0..6 {
            assert!(overrides.push(Override::default()).is_ok());
        }
        Preset::Fps.tune_keys(
            &mut settings,
            &mut overrides,
            Movement::find(&keymap()),
            |_, mm| hundredths(mm),
        );
        assert_eq!(settings.actuation, hundredths(1.0));
        assert!(settings.rapid_trigger_enabled);
        assert_eq!(settings.rapid_trigger.press, hundredths(0.2));
        assert_eq!(settings.rapid_trigger.release, None);
        assert!(settings.rapid_trigger.continuous);
        assert_eq!(
            overrides.get(1).and_then(|entry| entry.actuation),
            Some(hundredths(0.4))
        );
        assert_eq!(
            overrides.first().and_then(|entry| entry.actuation),
            Some(hundredths(3.0))
        );
        assert_eq!(overrides.get(2).and_then(|entry| entry.actuation), None);
        assert_eq!(
            overrides.get(5).map(|entry| entry.rapid_trigger),
            Some(RapidTriggerOverride::Profile)
        );
    }

    #[test]
    fn typing_preset_turns_rapid_trigger_off_and_clears_key_overrides() {
        let mut settings = keys::Settings {
            actuation: hundredths(0.4),
            rapid_trigger_enabled: true,
            rapid_trigger: RapidTrigger {
                press: hundredths(0.2),
                release: None,
                continuous: false,
            },
            tap_duration: TapDuration::FortyMilliseconds,
            scan_mode: ScanMode::Parallel,
            keyboard_in_gamepad_mode: false,
        };
        let mut overrides: keys::Overrides = heapless::Vec::new();
        for _ in 0..6 {
            let tuned = Override {
                actuation: Some(hundredths(0.1)),
                rapid_trigger: RapidTriggerOverride::On,
            };
            assert!(overrides.push(tuned).is_ok());
        }
        Preset::Typing.tune_keys(
            &mut settings,
            &mut overrides,
            Movement::find(&keymap()),
            |_, mm| hundredths(mm),
        );
        assert_eq!(settings.actuation, hundredths(1.5));
        assert!(!settings.rapid_trigger_enabled);
        assert!(overrides.iter().all(|entry| *entry == Override::default()));
    }

    #[test]
    fn racing_preset_binds_only_the_driving_keys() {
        let mut mapping = gamepad::Mapping {
            bindings: heapless::Vec::new(),
            exclusive: false,
        };
        for _ in 0..6 {
            assert!(mapping.bindings.push(Some(Control::ButtonA)).is_ok());
        }
        Preset::Racing.map_gamepad(&mut mapping, Movement::find(&keymap()));
        assert_eq!(
            mapping.bindings.as_slice(),
            &[
                None,
                Some(Control::RightTrigger),
                None,
                Some(Control::LeftStickLeft),
                Some(Control::LeftTrigger),
                Some(Control::LeftStickRight),
            ]
        );
    }

    #[test]
    fn typing_preset_replaces_the_effects_with_a_blue_wave() {
        let mut settings = lighting::Settings {
            brightness: 200,
            indicators: lighting::Indicators {
                caps_lock: lighting::Color::TRANSPARENT,
                scroll_lock: lighting::Color::TRANSPARENT,
                num_lock: lighting::Color::TRANSPARENT,
                layer_lock: lighting::Color::TRANSPARENT,
            },
            effects: heapless::Vec::new(),
        };
        Preset::Fps.tune_lighting(&mut settings);
        let unchanged: [Effect; 0] = [];
        assert_eq!(settings.effects.as_slice(), &unchanged);
        Preset::Typing.tune_lighting(&mut settings);
        assert_eq!(settings.effects.as_slice(), &[TYPING_WAVE]);
        assert_eq!(settings.brightness, 200);
    }
}
