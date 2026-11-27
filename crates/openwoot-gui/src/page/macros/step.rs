use gpui_kit::assets::IconName as Catalog;
use openwoot_protocol::{
    keys::Action,
    macros::{Step, Text},
};

use super::text;

pub(super) const DEFAULT_DELAY_MS: u16 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Tap,
    Press,
    Release,
    Delay,
    Text,
    MouseMove,
    Scroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Part {
    Delay,
    Text,
    MoveX,
    MoveY,
    Vertical,
    Horizontal,
}

impl Kind {
    pub(super) const ALL: [Self; 7] = [
        Self::Tap,
        Self::Press,
        Self::Release,
        Self::Delay,
        Self::Text,
        Self::MouseMove,
        Self::Scroll,
    ];
    pub(super) const KEYS: [Self; 3] = [Self::Tap, Self::Press, Self::Release];

    pub(super) const fn of(step: &Step) -> Self {
        match step {
            Step::Tap(_) => Self::Tap,
            Step::Press(_) => Self::Press,
            Step::Release(_) => Self::Release,
            Step::Delay(_) => Self::Delay,
            Step::Text(_) => Self::Text,
            Step::MouseMove { .. } => Self::MouseMove,
            Step::Scroll { .. } => Self::Scroll,
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Tap => "Tap",
            Self::Press => "Press",
            Self::Release => "Release",
            Self::Delay => "Wait",
            Self::Text => "Type",
            Self::MouseMove => "Move",
            Self::Scroll => "Scroll",
        }
    }

    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Tap => "Key tap",
            Self::Press => "Key press",
            Self::Release => "Key release",
            Self::Delay => "Delay",
            Self::Text => "Text",
            Self::MouseMove => "Mouse movement",
            Self::Scroll => "Scroll",
        }
    }

    pub(super) const fn icon(self) -> Catalog {
        match self {
            Self::Tap => Catalog::ArrowDownUp,
            Self::Press => Catalog::ArrowDownToLine,
            Self::Release => Catalog::ArrowUpFromLine,
            Self::Delay => Catalog::Timer,
            Self::Text => Catalog::Type,
            Self::MouseMove => Catalog::Move,
            Self::Scroll => Catalog::ChevronsUpDown,
        }
    }

    pub(super) const fn is_key(self) -> bool {
        matches!(self, Self::Tap | Self::Press | Self::Release)
    }

    pub(super) const fn with_action(self, action: Action) -> Option<Step> {
        match self {
            Self::Tap => Some(Step::Tap(action)),
            Self::Press => Some(Step::Press(action)),
            Self::Release => Some(Step::Release(action)),
            Self::Delay | Self::Text | Self::MouseMove | Self::Scroll => None,
        }
    }

    pub(super) const fn blank(self) -> Option<Step> {
        match self {
            Self::Delay => Some(Step::Delay(DEFAULT_DELAY_MS)),
            Self::Text => Some(Step::Text(Text::new())),
            Self::MouseMove => Some(Step::MouseMove { x: 0, y: 0 }),
            Self::Scroll => Some(Step::Scroll {
                vertical: 0,
                horizontal: 0,
            }),
            Self::Tap | Self::Press | Self::Release => None,
        }
    }

    pub(super) const fn parts(self) -> &'static [Part] {
        match self {
            Self::Delay => &[Part::Delay],
            Self::Text => &[Part::Text],
            Self::MouseMove => &[Part::MoveX, Part::MoveY],
            Self::Scroll => &[Part::Vertical, Part::Horizontal],
            Self::Tap | Self::Press | Self::Release => &[],
        }
    }
}

impl Part {
    pub(super) const fn prefix(self) -> Option<&'static str> {
        match self {
            Self::MoveX => Some("X"),
            Self::MoveY => Some("Y"),
            Self::Vertical => Some("↕"),
            Self::Horizontal => Some("↔"),
            Self::Delay | Self::Text => None,
        }
    }

    pub(super) const fn unit(self) -> Option<&'static str> {
        match self {
            Self::Delay => Some("ms"),
            Self::MoveY => Some("px"),
            Self::Text | Self::MoveX | Self::Vertical | Self::Horizontal => None,
        }
    }

    pub(super) const fn help(self) -> &'static str {
        match self {
            Self::Delay => "How long to wait, in milliseconds",
            Self::Text => {
                "Typed for a US, UK or JIS system layout, with \\n for a new line and \\t for a tab"
            }
            Self::MoveX => "Pixels to the right, negative moves left",
            Self::MoveY => "Pixels down, negative moves up",
            Self::Vertical => "Notches up, negative scrolls down",
            Self::Horizontal => "Notches right, negative scrolls left",
        }
    }

    pub(super) fn shown(self, step: &Step) -> String {
        match (self, step) {
            (Self::Delay, Step::Delay(milliseconds)) => milliseconds.to_string(),
            (Self::Text, Step::Text(typed)) => text::shown(typed),
            (Self::MoveX, Step::MouseMove { x, .. }) => x.to_string(),
            (Self::MoveY, Step::MouseMove { y, .. }) => y.to_string(),
            (Self::Vertical, Step::Scroll { vertical, .. }) => vertical.to_string(),
            (Self::Horizontal, Step::Scroll { horizontal, .. }) => horizontal.to_string(),
            _ => String::new(),
        }
    }

    pub(super) fn applied(self, step: &Step, typed: &str) -> Result<Step, String> {
        let typed = typed.trim();
        let mut changed = step.clone();
        match (self, &mut changed) {
            (Self::Delay, Step::Delay(milliseconds)) => {
                *milliseconds = typed
                    .parse()
                    .map_err(|_| format!("Enter a delay from 0 to {} ms", u16::MAX))?;
            }
            (Self::Text, Step::Text(value)) => {
                *value = text::parsed(typed).map_err(text::Problem::message)?;
            }
            (Self::MoveX, Step::MouseMove { x: distance, .. })
            | (Self::MoveY, Step::MouseMove { y: distance, .. }) => {
                *distance = typed
                    .parse()
                    .map_err(|_| format!("Enter a distance from {} to {}", i16::MIN, i16::MAX))?;
            }
            (
                Self::Vertical,
                Step::Scroll {
                    vertical: notches, ..
                },
            )
            | (
                Self::Horizontal,
                Step::Scroll {
                    horizontal: notches,
                    ..
                },
            ) => {
                *notches = typed
                    .parse()
                    .ok()
                    .filter(|amount| *amount != i8::MIN)
                    .ok_or_else(|| format!("Enter notches from {} to {}", -i8::MAX, i8::MAX))?;
            }
            _ => {}
        }
        Ok(changed)
    }
}

pub(super) const fn action(step: &Step) -> Option<Action> {
    match step {
        Step::Tap(action) | Step::Press(action) | Step::Release(action) => Some(*action),
        Step::Delay(_) | Step::Text(_) | Step::MouseMove { .. } | Step::Scroll { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: Action = Action::Keyboard(0x04);

    #[test]
    fn key_steps_change_their_kind_but_keep_their_key() {
        let step = Step::Press(KEY_A);
        let kind = Kind::of(&step);
        assert!(kind.is_key());
        assert_eq!(action(&step), Some(KEY_A));
        assert_eq!(Kind::Release.with_action(KEY_A), Some(Step::Release(KEY_A)));
        assert_eq!(Kind::Delay.with_action(KEY_A), None);
        assert!(Kind::KEYS.iter().all(|kind| kind.blank().is_none()));
    }

    #[test]
    fn every_kind_starts_from_a_step_of_its_own_kind() {
        for kind in Kind::ALL {
            let step = kind.blank().or_else(|| kind.with_action(KEY_A)).unwrap();
            assert_eq!(Kind::of(&step), kind);
            for part in kind.parts() {
                let shown = part.shown(&step);
                assert_eq!(part.applied(&step, &shown), Ok(step.clone()));
            }
        }
    }

    #[test]
    fn values_are_read_from_typed_text() {
        let moved = Step::MouseMove { x: 3, y: 4 };
        assert_eq!(
            Part::MoveY.applied(&moved, " -120 "),
            Ok(Step::MouseMove { x: 3, y: -120 })
        );
        assert_eq!(
            Part::Delay.applied(&Step::Delay(5), "250"),
            Ok(Step::Delay(250))
        );
        let scrolled = Step::Scroll {
            vertical: 1,
            horizontal: 0,
        };
        assert_eq!(
            Part::Horizontal.applied(&scrolled, "-3"),
            Ok(Step::Scroll {
                vertical: 1,
                horizontal: -3
            })
        );
        assert_eq!(
            Part::Text.applied(&Step::Text(Text::new()), "Hi\\n"),
            Ok(Step::Text(Text::try_from("Hi\n").unwrap()))
        );
    }

    #[test]
    fn values_out_of_range_or_untypable_are_refused() {
        assert!(Part::Delay.applied(&Step::Delay(5), "70000").is_err());
        assert!(Part::Delay.applied(&Step::Delay(5), "-1").is_err());
        let still = Step::Scroll {
            vertical: 0,
            horizontal: 0,
        };
        assert!(Part::Vertical.applied(&still, "200").is_err());
        assert!(Part::Vertical.applied(&still, "-128").is_err());
        assert_eq!(
            Part::Vertical.applied(&still, "-127"),
            Ok(Step::Scroll {
                vertical: -127,
                horizontal: 0
            })
        );
        assert!(
            Part::Text
                .applied(&Step::Text(Text::new()), "grüße")
                .is_err()
        );
    }
}
