use gpui_kit::{
    AnyElement, FontWeight, Hsla, IntoElement, ParentElement as _, Pixels, Styled as _, div,
};
use openwoot_protocol::{
    advanced::{
        Behaviour, DepthPriority, DynamicKeystroke, Interrupt, ModTap, Resolution, STROKES, Socd,
        Stroke, StrokeEvent,
    },
    gamepad::Control,
    keys::{Action, Index},
};

use crate::{keyboard::Glyph, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    DynamicKeystroke,
    ModTap,
    Toggle,
    DepthPriority,
    Socd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Point {
    Press,
    BottomOut,
    Lift,
    Release,
}

pub(crate) struct Choice<T> {
    pub(crate) value: T,
    pub(crate) title: String,
    pub(crate) description: String,
}

impl Point {
    pub(crate) const ALL: [Self; 4] = [Self::Press, Self::BottomOut, Self::Lift, Self::Release];

    pub(crate) const fn help(self) -> &'static str {
        match self {
            Self::Press => "The key passes its actuation point",
            Self::BottomOut => "The key is fully pressed",
            Self::Lift => "The key rises from full press",
            Self::Release => "The key is released",
        }
    }

    pub(crate) const fn event(self, stroke: Stroke) -> StrokeEvent {
        match self {
            Self::Press => stroke.on_press,
            Self::BottomOut => stroke.on_bottom_out,
            Self::Lift => stroke.on_lift,
            Self::Release => stroke.on_release,
        }
    }

    pub(crate) const fn set(self, stroke: &mut Stroke, event: StrokeEvent) {
        match self {
            Self::Press => stroke.on_press = event,
            Self::BottomOut => stroke.on_bottom_out = event,
            Self::Lift => stroke.on_lift = event,
            Self::Release => stroke.on_release = event,
        }
    }
}

impl Kind {
    pub(crate) const fn of(behaviour: &Behaviour) -> Self {
        match behaviour {
            Behaviour::DynamicKeystroke(_) => Self::DynamicKeystroke,
            Behaviour::ModTap(_) => Self::ModTap,
            Behaviour::Toggle(_) => Self::Toggle,
            Behaviour::DepthPriority(_) => Self::DepthPriority,
            Behaviour::Socd(_) => Self::Socd,
        }
    }

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::DynamicKeystroke => "Dynamic Keystroke (DKS)",
            Self::ModTap => "Mod Tap",
            Self::Toggle => "Toggle Key",
            Self::DepthPriority => "Rappy Snappy",
            Self::Socd => "Snappy Tappy (SOCD)",
        }
    }

    pub(crate) const fn short_title(self) -> &'static str {
        match self {
            Self::DynamicKeystroke => "Dynamic Keystroke",
            Self::ModTap => "Mod Tap",
            Self::Toggle => "Toggle Key",
            Self::DepthPriority => "Rappy Snappy",
            Self::Socd => "Snappy Tappy",
        }
    }

    pub(crate) const fn badge(self) -> &'static str {
        match self {
            Self::DynamicKeystroke => "DKS",
            Self::ModTap => "MT",
            Self::Toggle => "TGL",
            Self::DepthPriority => "RS",
            Self::Socd => "SOCD",
        }
    }

    pub(crate) const fn help(self) -> &'static str {
        match self {
            Self::DynamicKeystroke => "Up to 4 actions on 4 parts of one key press",
            Self::ModTap => "One action when the key is held, another when it is tapped",
            Self::Toggle => "Tap to lock an action on or off, hold for the normal behaviour",
            Self::DepthPriority => "Of 2 keys, the one pressed further down activates",
            Self::Socd => "Of 2 keys pressed together, your setting decides which activates",
        }
    }

    pub(crate) fn glyph(self, ink: Hsla) -> Glyph {
        Glyph::new(move |height| self.mark(height, ink))
    }

    pub(crate) fn mark(self, height: Pixels, ink: Hsla) -> AnyElement {
        div()
            .h(height * 0.44)
            .px(height * 0.1)
            .flex()
            .items_center()
            .rounded(height * 0.08)
            .bg(ink)
            .text_color(theme::keycaps().top)
            .text_size(height * if self.badge().len() > 3 { 0.2 } else { 0.26 })
            .font_weight(FontWeight::EXTRA_BOLD)
            .italic()
            .child(self.badge())
            .into_any_element()
    }

    pub(crate) const fn key_count(self) -> usize {
        match self {
            Self::DynamicKeystroke | Self::ModTap | Self::Toggle => 1,
            Self::DepthPriority | Self::Socd => 2,
        }
    }

    pub(crate) const fn owns_actions(self) -> bool {
        matches!(self, Self::DynamicKeystroke | Self::ModTap | Self::Toggle)
    }

    pub(crate) fn behaviour(self, action: Action, keys: &[Index]) -> Option<Behaviour> {
        Some(match self {
            Self::DynamicKeystroke => Behaviour::DynamicKeystroke(held_through(action)),
            Self::ModTap => Behaviour::ModTap(ModTap {
                tap: Action::None,
                hold: action,
                hold_time_ms: ModTap::DEFAULT_HOLD_TIME_MS,
                interrupt: Interrupt::Auto,
            }),
            Self::Toggle => Behaviour::Toggle(action),
            Self::DepthPriority => Behaviour::DepthPriority(DepthPriority {
                other: *keys.get(1)?,
                both_bottomed_out: Resolution::Both,
            }),
            Self::Socd => Behaviour::Socd(Socd {
                other: *keys.get(1)?,
                resolution: Resolution::LastPressed,
                both_when_bottomed_out: false,
            }),
        })
    }
}

pub(crate) const fn held_through(action: Action) -> DynamicKeystroke {
    let mut strokes = [Stroke {
        action: Action::None,
        on_press: StrokeEvent::Nothing,
        on_bottom_out: StrokeEvent::Nothing,
        on_lift: StrokeEvent::Nothing,
        on_release: StrokeEvent::Nothing,
    }; STROKES];
    strokes[0] = Stroke {
        action,
        on_press: StrokeEvent::Press,
        on_bottom_out: StrokeEvent::Nothing,
        on_lift: StrokeEvent::Nothing,
        on_release: StrokeEvent::Release,
    };
    DynamicKeystroke {
        strokes,
        bottom_out: None,
    }
}

pub(crate) const fn follows_travel(action: Action) -> bool {
    matches!(
        action,
        Action::Gamepad(
            Control::LeftStickUp
                | Control::LeftStickDown
                | Control::LeftStickLeft
                | Control::LeftStickRight
                | Control::RightStickUp
                | Control::RightStickDown
                | Control::RightStickLeft
                | Control::RightStickRight
                | Control::LeftTrigger
                | Control::RightTrigger
                | Control::Accelerator
                | Control::Rudder
                | Control::Trigger
                | Control::RxNegative
                | Control::RxPositive
                | Control::RyNegative
                | Control::RyPositive
        )
    )
}

pub(crate) fn interrupts() -> [Choice<Interrupt>; 4] {
    [
        choice(
            Interrupt::Auto,
            "Hold key, if it is a modifier",
            "Uses the hold action when it is a modifier like Shift, Ctrl or Alt",
        ),
        choice(
            Interrupt::Tap,
            "Tap key",
            "Always uses the tap action; suits home row modifiers",
        ),
        choice(Interrupt::Hold, "Hold key", "Always uses the hold action"),
        choice(
            Interrupt::Ignore,
            "Never",
            "Turns just-in-time off; the key waits for the full hold duration",
        ),
    ]
}

pub(crate) fn socd_resolutions(first: &str, second: &str) -> [Choice<Resolution>; 5] {
    [
        choice(
            Resolution::LastPressed,
            "Last input priority",
            "The key pressed last overrides the other",
        ),
        choice(
            Resolution::First,
            &format!("Absolute priority ({first})"),
            &format!("{first} always takes priority over {second}"),
        ),
        choice(
            Resolution::Second,
            &format!("Absolute priority ({second})"),
            &format!("{second} always takes priority over {first}"),
        ),
        choice(
            Resolution::Neither,
            "Neutral",
            "When both keys are pressed, neither registers",
        ),
        choice(
            Resolution::Both,
            "No cleaning",
            "Both keys register, as without Snappy Tappy",
        ),
    ]
}

pub(crate) fn bottomed_out_resolutions(first: &str, second: &str) -> [Choice<Resolution>; 5] {
    [
        choice(Resolution::Both, "Both", "Both keys register"),
        choice(Resolution::Neither, "Neither", "Neither key registers"),
        choice(Resolution::First, first, &format!("Only {first} registers")),
        choice(
            Resolution::Second,
            second,
            &format!("Only {second} registers"),
        ),
        choice(
            Resolution::LastPressed,
            "Last pressed",
            "The key pressed last registers",
        ),
    ]
}

fn choice<T>(value: T, title: &str, description: &str) -> Choice<T> {
    Choice {
        value,
        title: title.to_owned(),
        description: description.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_advanced_keys_start_from_the_key_action_like_wootility() {
        let action = Action::Keyboard(0x1A);
        assert_eq!(
            Kind::DynamicKeystroke.behaviour(action, &[Index::new(3)]),
            Some(Behaviour::DynamicKeystroke(held_through(action)))
        );
        let keystroke = held_through(action);
        assert_eq!(
            keystroke.strokes.first().map(|stroke| stroke.action),
            Some(action)
        );
        assert_eq!(
            keystroke
                .strokes
                .first()
                .map(|stroke| (stroke.on_press, stroke.on_release)),
            Some((StrokeEvent::Press, StrokeEvent::Release))
        );
        assert!(
            keystroke
                .strokes
                .iter()
                .skip(1)
                .all(|stroke| stroke.action == Action::None)
        );
        assert_eq!(
            Kind::ModTap.behaviour(action, &[Index::new(3)]),
            Some(Behaviour::ModTap(ModTap {
                tap: Action::None,
                hold: action,
                hold_time_ms: 200,
                interrupt: Interrupt::Auto,
            }))
        );
        assert_eq!(
            Kind::Socd.behaviour(action, &[Index::new(3), Index::new(5)]),
            Some(Behaviour::Socd(Socd {
                other: Index::new(5),
                resolution: Resolution::LastPressed,
                both_when_bottomed_out: false,
            }))
        );
        assert_eq!(
            Kind::DepthPriority.behaviour(action, &[Index::new(3)]),
            None
        );
    }

    #[test]
    fn every_kind_round_trips_through_its_behaviour() {
        let keys = [Index::new(1), Index::new(2)];
        for kind in [
            Kind::DynamicKeystroke,
            Kind::ModTap,
            Kind::Toggle,
            Kind::DepthPriority,
            Kind::Socd,
        ] {
            let behaviour = kind.behaviour(Action::Keyboard(0x04), &keys);
            assert_eq!(behaviour.as_ref().map(Kind::of), Some(kind));
        }
    }
}
