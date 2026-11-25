use openwoot_protocol::{
    advanced::{
        self, Behaviour, DepthPriority, DynamicKeystroke, Interrupt, ModTap, Resolution, Socd,
        Stroke, StrokeEvent,
    },
    keys::{Action, Index, Keymap, Layer},
};

use super::kind::Kind;

const KEY_W: Action = Action::Keyboard(0x1A);
const KEY_A: Action = Action::Keyboard(0x04);
const KEY_S: Action = Action::Keyboard(0x16);
const KEY_D: Action = Action::Keyboard(0x07);
const TAB: Action = Action::Keyboard(0x2B);
const CAPS_LOCK: Action = Action::Keyboard(0x39);
const MENU: Action = Action::Keyboard(0x65);
const RIGHT: Action = Action::Keyboard(0x4F);
const LEFT: Action = Action::Keyboard(0x50);
const DOWN: Action = Action::Keyboard(0x51);
const UP: Action = Action::Keyboard(0x52);
const LEFT_SHIFT: Action = Action::Keyboard(0xE1);
const LEFT_ALT: Action = Action::Keyboard(0xE2);
const RIGHT_CONTROL: Action = Action::Keyboard(0xE4);
const RIGHT_SHIFT: Action = Action::Keyboard(0xE5);
const FN_1: Action = Action::Layer(Layer::One);
const FN_2: Action = Action::Layer(Layer::Two);
const UNUSED: Stroke = stroke(
    Action::None,
    [
        StrokeEvent::Nothing,
        StrokeEvent::Nothing,
        StrokeEvent::Nothing,
        StrokeEvent::Nothing,
    ],
);

pub(crate) struct Preset {
    pub(crate) kind: Kind,
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    parts: &'static [Part],
}

struct Part {
    key: Action,
    other: Option<Action>,
    template: Template,
}

#[derive(Debug, Clone, Copy)]
enum Template {
    Keystroke(DynamicKeystroke),
    ModTap(ModTap),
    Toggle(Action),
    DepthPriority(Resolution),
    Socd(Resolution, bool),
}

const STRAFE: &str = "Instant strafe switching";
const STRAFE_KEYS: &str = "Switch between A and D instantly without the normal input delay.";
const STRAFE_ARROWS: &str =
    "Switch between left and right instantly without the normal input delay.";
const ALT_TAB: &str = "Reduce accidental Alt + Tab";
const ALT_TAB_DESCRIPTION: &str = "Alt+Tab only triggers when both keys are fully bottomed out.";
const RUNNING: &str = "Key Response for Running in Games";
const RUNNING_DESCRIPTION: &str = "Enable shift at the full press for running";

pub(crate) const PRESETS: &[Preset] = &[
    Preset {
        kind: Kind::DepthPriority,
        title: STRAFE,
        description: STRAFE_KEYS,
        parts: &[pair(
            KEY_A,
            KEY_D,
            Template::DepthPriority(Resolution::Both),
        )],
    },
    Preset {
        kind: Kind::DepthPriority,
        title: ALT_TAB,
        description: ALT_TAB_DESCRIPTION,
        parts: &[pair(
            LEFT_ALT,
            TAB,
            Template::DepthPriority(Resolution::Both),
        )],
    },
    Preset {
        kind: Kind::DepthPriority,
        title: STRAFE,
        description: STRAFE_ARROWS,
        parts: &[pair(LEFT, RIGHT, Template::DepthPriority(Resolution::Both))],
    },
    Preset {
        kind: Kind::Socd,
        title: STRAFE,
        description: STRAFE_KEYS,
        parts: &[pair(
            KEY_A,
            KEY_D,
            Template::Socd(Resolution::LastPressed, false),
        )],
    },
    Preset {
        kind: Kind::Socd,
        title: ALT_TAB,
        description: ALT_TAB_DESCRIPTION,
        parts: &[pair(
            LEFT_ALT,
            TAB,
            Template::Socd(Resolution::Neither, true),
        )],
    },
    Preset {
        kind: Kind::Socd,
        title: STRAFE,
        description: STRAFE_ARROWS,
        parts: &[pair(
            LEFT,
            RIGHT,
            Template::Socd(Resolution::LastPressed, false),
        )],
    },
    Preset {
        kind: Kind::DynamicKeystroke,
        title: RUNNING,
        description: RUNNING_DESCRIPTION,
        parts: &[
            running(KEY_W),
            running(KEY_A),
            running(KEY_S),
            running(KEY_D),
        ],
    },
    Preset {
        kind: Kind::DynamicKeystroke,
        title: RUNNING,
        description: RUNNING_DESCRIPTION,
        parts: &[running(KEY_W)],
    },
    Preset {
        kind: Kind::ModTap,
        title: "Mod Tap Arrow Keys",
        description: "Add arrow keys to your keyboard in one click",
        parts: &[
            mod_tap(MENU, LEFT, MENU),
            mod_tap(RIGHT_SHIFT, UP, RIGHT_SHIFT),
            mod_tap(FN_1, RIGHT, FN_1),
            mod_tap(RIGHT_CONTROL, DOWN, RIGHT_CONTROL),
        ],
    },
    Preset {
        kind: Kind::ModTap,
        title: "Mod Tap Fn 2 on Caps Lock",
        description: "Add Fn 2 as a hold key on Caps Lock in one click",
        parts: &[mod_tap(CAPS_LOCK, CAPS_LOCK, FN_2)],
    },
    Preset {
        kind: Kind::Toggle,
        title: "Toggle Function Layer 1",
        description: "Toggle Fn 1 on and off without holding the key",
        parts: &[single(FN_1, Template::Toggle(FN_1))],
    },
    Preset {
        kind: Kind::Toggle,
        title: "Toggle Function Layer 2 on Caps Lock",
        description: "Swap between Fn 2 and Caps Lock in one click",
        parts: &[single(CAPS_LOCK, Template::Toggle(FN_2))],
    },
];

impl Preset {
    pub(crate) fn chips(&self) -> Vec<Action> {
        self.parts
            .iter()
            .flat_map(|part| match part.template {
                Template::ModTap(mod_tap) if mod_tap.tap == part.key => vec![mod_tap.hold],
                Template::ModTap(mod_tap) => vec![mod_tap.tap],
                Template::Toggle(action) => vec![action],
                Template::Keystroke(_) | Template::DepthPriority(_) | Template::Socd(..) => {
                    core::iter::once(part.key).chain(part.other).collect()
                }
            })
            .collect()
    }

    pub(crate) fn offered(&self, base: &Keymap) -> bool {
        let added: Vec<Action> = self
            .parts
            .iter()
            .filter_map(|part| match part.template {
                Template::ModTap(mod_tap) if mod_tap.tap != part.key => Some(mod_tap.tap),
                _ => None,
            })
            .collect();
        added.is_empty()
            || added
                .iter()
                .any(|action| key_bound_to(base, *action).is_none())
    }

    pub(crate) fn entries(&self, base: &Keymap) -> Result<Vec<advanced::Key>, Vec<Action>> {
        let mut missing = Vec::new();
        let mut entries = Vec::with_capacity(self.parts.len());
        for part in self.parts {
            let key = key_bound_to(base, part.key);
            let other = part
                .other
                .map(|action| (action, key_bound_to(base, action)));
            if key.is_none() {
                missing.push(part.key);
            }
            if let Some((action, None)) = other {
                missing.push(action);
            }
            let other = other.and_then(|(_, key)| key);
            if let Some(key) = key
                && let Some(behaviour) = part.template.behaviour(other)
            {
                entries.push(advanced::Key {
                    key,
                    layer: Layer::Base,
                    behaviour,
                });
            }
        }
        if missing.is_empty() {
            Ok(entries)
        } else {
            Err(missing)
        }
    }
}

impl Template {
    const fn behaviour(self, other: Option<Index>) -> Option<Behaviour> {
        match (self, other) {
            (Self::Keystroke(keystroke), _) => Some(Behaviour::DynamicKeystroke(keystroke)),
            (Self::ModTap(mod_tap), _) => Some(Behaviour::ModTap(mod_tap)),
            (Self::Toggle(action), _) => Some(Behaviour::Toggle(action)),
            (Self::DepthPriority(both_bottomed_out), Some(other)) => {
                Some(Behaviour::DepthPriority(DepthPriority {
                    other,
                    both_bottomed_out,
                }))
            }
            (Self::Socd(resolution, both_when_bottomed_out), Some(other)) => {
                Some(Behaviour::Socd(Socd {
                    other,
                    resolution,
                    both_when_bottomed_out,
                }))
            }
            (Self::DepthPriority(_) | Self::Socd(..), None) => None,
        }
    }
}

fn key_bound_to(base: &Keymap, action: Action) -> Option<Index> {
    base.iter()
        .position(|bound| *bound == action)
        .and_then(|key| u8::try_from(key).ok())
        .map(Index::new)
}

const fn stroke(action: Action, events: [StrokeEvent; 4]) -> Stroke {
    let [on_press, on_bottom_out, on_lift, on_release] = events;
    Stroke {
        action,
        on_press,
        on_bottom_out,
        on_lift,
        on_release,
    }
}

const fn running(key: Action) -> Part {
    single(
        key,
        Template::Keystroke(DynamicKeystroke {
            strokes: [
                stroke(
                    key,
                    [
                        StrokeEvent::Press,
                        StrokeEvent::Nothing,
                        StrokeEvent::Nothing,
                        StrokeEvent::Release,
                    ],
                ),
                stroke(
                    LEFT_SHIFT,
                    [
                        StrokeEvent::Nothing,
                        StrokeEvent::Press,
                        StrokeEvent::Release,
                        StrokeEvent::Nothing,
                    ],
                ),
                UNUSED,
                UNUSED,
            ],
            bottom_out: None,
        }),
    )
}

const fn mod_tap(key: Action, tap: Action, hold: Action) -> Part {
    single(
        key,
        Template::ModTap(ModTap {
            tap,
            hold,
            hold_time_ms: ModTap::DEFAULT_HOLD_TIME_MS,
            interrupt: Interrupt::Auto,
        }),
    )
}

const fn single(key: Action, template: Template) -> Part {
    Part {
        key,
        other: None,
        template,
    }
}

const fn pair(key: Action, other: Action, template: Template) -> Part {
    Part {
        key,
        other: Some(other),
        template,
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::{
        configuration::{Capabilities, Validate as _},
        heapless,
    };

    use super::*;

    const COLUMNS: u8 = 17;

    fn base() -> Keymap {
        let mut keymap: Keymap = (0..u16::from(COLUMNS) * 6).map(|_| Action::None).collect();
        let bound = [
            KEY_W,
            KEY_A,
            KEY_S,
            KEY_D,
            TAB,
            CAPS_LOCK,
            MENU,
            RIGHT,
            LEFT,
            DOWN,
            UP,
            LEFT_SHIFT,
            LEFT_ALT,
            RIGHT_CONTROL,
            RIGHT_SHIFT,
            FN_1,
        ];
        for (slot, action) in keymap.iter_mut().skip(20).zip(bound) {
            *slot = action;
        }
        keymap
    }

    fn capabilities() -> Capabilities {
        Capabilities {
            key_rows: 6,
            key_columns: COLUMNS,
            profiles: 4,
            linked_profiles: 0,
            layouts: heapless::Vec::new(),
            lightbar_leds: 0,
            host_signals: false,
            switch_models: 0,
            edition: None,
        }
    }

    #[test]
    fn every_preset_finds_its_keys_and_passes_the_firmware_rules() {
        let base = base();
        for preset in PRESETS {
            let entries = preset.entries(&base);
            assert!(entries.is_ok(), "{} misses {:?}", preset.title, entries);
            let entries = entries.unwrap_or_default();
            assert_eq!(entries.len(), preset.parts.len());
            let keys: advanced::Keys = heapless::Vec::from_slice(&entries).unwrap_or_default();
            assert_eq!(keys.validate(&capabilities()), Ok(()), "{}", preset.title);
            assert!(
                entries
                    .iter()
                    .all(|entry| Kind::of(&entry.behaviour) == preset.kind),
                "{} builds another kind",
                preset.title
            );
        }
    }

    #[test]
    fn mod_tap_arrows_keep_the_modifier_on_hold() {
        let base = base();
        let Some(preset) = PRESETS
            .iter()
            .find(|preset| preset.title == "Mod Tap Arrow Keys")
        else {
            unreachable!("the arrow preset is listed");
        };
        let entries = preset.entries(&base).unwrap_or_default();
        let menu = key_bound_to(&base, MENU);
        let entry = entries.iter().find(|entry| Some(entry.key) == menu);
        assert_eq!(
            entry.map(|entry| entry.behaviour),
            Some(Behaviour::ModTap(ModTap {
                tap: LEFT,
                hold: MENU,
                hold_time_ms: 200,
                interrupt: Interrupt::Auto,
            }))
        );
    }

    #[test]
    fn a_preset_names_the_bindings_it_cannot_find() {
        let mut base = base();
        for slot in &mut base {
            if *slot == KEY_D {
                *slot = Action::None;
            }
        }
        let socd = PRESETS
            .iter()
            .find(|preset| preset.kind == Kind::Socd && preset.title == STRAFE);
        assert_eq!(
            socd.map(|preset| preset.entries(&base)),
            Some(Err(vec![KEY_D]))
        );
    }

    #[test]
    fn preset_cards_show_the_keys_or_actions_they_bind() {
        let chips = |title: &str, kind: Kind| {
            PRESETS
                .iter()
                .find(|preset| preset.title == title && preset.kind == kind)
                .map(Preset::chips)
        };
        assert_eq!(chips(STRAFE, Kind::Socd), Some(vec![KEY_A, KEY_D]));
        assert_eq!(
            chips(ALT_TAB, Kind::DepthPriority),
            Some(vec![LEFT_ALT, TAB])
        );
        assert_eq!(
            chips("Mod Tap Arrow Keys", Kind::ModTap),
            Some(vec![LEFT, UP, RIGHT, DOWN])
        );
        assert_eq!(
            chips("Mod Tap Fn 2 on Caps Lock", Kind::ModTap),
            Some(vec![FN_2])
        );
        assert_eq!(
            chips("Toggle Function Layer 2 on Caps Lock", Kind::Toggle),
            Some(vec![FN_2])
        );
    }

    #[test]
    fn arrow_presets_are_offered_only_without_arrow_keys() {
        let arrows = PRESETS
            .iter()
            .find(|preset| preset.title == "Mod Tap Arrow Keys");
        let caps = PRESETS
            .iter()
            .find(|preset| preset.title == "Mod Tap Fn 2 on Caps Lock");
        let with_arrows = base();
        let mut without_arrows = base();
        for slot in &mut without_arrows {
            if [LEFT, RIGHT, UP, DOWN].contains(slot) {
                *slot = Action::None;
            }
        }
        assert_eq!(
            arrows.map(|preset| preset.offered(&with_arrows)),
            Some(false)
        );
        assert_eq!(
            arrows.map(|preset| preset.offered(&without_arrows)),
            Some(true)
        );
        assert_eq!(caps.map(|preset| preset.offered(&with_arrows)), Some(true));
    }

    #[test]
    fn running_presses_shift_between_bottom_out_and_lift() {
        let Template::Keystroke(keystroke) = running(KEY_W).template else {
            unreachable!("running is a dynamic keystroke");
        };
        let [walk, sprint, ..] = keystroke.strokes;
        assert_eq!(
            (walk.action, walk.on_press, walk.on_release),
            (KEY_W, StrokeEvent::Press, StrokeEvent::Release)
        );
        assert_eq!(
            (sprint.action, sprint.on_bottom_out, sprint.on_lift),
            (LEFT_SHIFT, StrokeEvent::Press, StrokeEvent::Release)
        );
    }
}
