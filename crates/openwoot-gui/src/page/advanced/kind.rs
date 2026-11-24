use openwoot_protocol::{
    advanced::{
        Behaviour, DepthPriority, DynamicKeystroke, Interrupt, ModTap, Resolution, STROKES, Socd,
        Stroke, StrokeEvent,
    },
    gamepad::Control,
    keys::{Action, Index},
};

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

pub(crate) const STROKE_EVENTS: [StrokeEvent; 4] = [
    StrokeEvent::Nothing,
    StrokeEvent::Press,
    StrokeEvent::Release,
    StrokeEvent::Tap,
];

impl Point {
    pub(crate) const ALL: [Self; 4] = [Self::Press, Self::BottomOut, Self::Lift, Self::Release];

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Press => "Start of press",
            Self::BottomOut => "Fully pressed",
            Self::Lift => "Leaving full press",
            Self::Release => "Fully released",
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
    pub(crate) const ALL: [Self; 5] = [
        Self::DynamicKeystroke,
        Self::ModTap,
        Self::Toggle,
        Self::DepthPriority,
        Self::Socd,
    ];

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

    pub(crate) const fn badge(self) -> &'static str {
        match self {
            Self::DynamicKeystroke => "DKS",
            Self::ModTap => "Mod Tap",
            Self::Toggle => "Toggle",
            Self::DepthPriority => "Rappy",
            Self::Socd => "SOCD",
        }
    }

    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::DynamicKeystroke => {
                "4 different actions on a single key based on key position. Activate 1 up to 4 \
                 bindings on 4 different parts of the key press."
            }
            Self::ModTap => {
                "2 different actions on a single key based on press behaviour. Hold the key down \
                 for the first action or tap the key for the second action."
            }
            Self::Toggle => {
                "Lock an action between its on and off state by tapping the key. Activate the \
                 normal key behaviour by holding down the key."
            }
            Self::DepthPriority => {
                "Rappy Snappy monitors the 2 selected keys and activates whichever key is pressed \
                 down further."
            }
            Self::Socd => {
                "Snappy Tappy (SOCD Cleaning) monitors the 2 selected keys and activates them \
                 based on your chosen settings."
            }
        }
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

pub(crate) const fn event_label(event: StrokeEvent) -> &'static str {
    match event {
        StrokeEvent::Nothing => "·",
        StrokeEvent::Press => "Press",
        StrokeEvent::Release => "Release",
        StrokeEvent::Tap => "Tap",
    }
}

pub(crate) const fn event_description(event: StrokeEvent) -> &'static str {
    match event {
        StrokeEvent::Nothing => "Nothing: the action keeps its state",
        StrokeEvent::Press => "Press: hold the action down until a later Release",
        StrokeEvent::Release => "Release: let go of the action",
        StrokeEvent::Tap => "Tap: press and release the action once",
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
    let choice = |value, title: &str, description: &str| Choice {
        value,
        title: title.to_owned(),
        description: description.to_owned(),
    };
    [
        choice(
            Interrupt::Auto,
            "Hold Key, if it is a modifier",
            "Uses the hold function when it's a modifier like Shift, Ctrl, or Alt, or a Fn layer.",
        ),
        choice(
            Interrupt::Tap,
            "Tap Key",
            "Always uses the tap function. Use this if you're setting up home row modifiers.",
        ),
        choice(
            Interrupt::Hold,
            "Hold Key",
            "Always uses the hold function.",
        ),
        choice(
            Interrupt::Ignore,
            "Never",
            "Disables Just-in-time. Your key waits for the full delay before activating.",
        ),
    ]
}

pub(crate) fn socd_resolutions(first: &str, second: &str) -> [Choice<Resolution>; 5] {
    [
        Choice {
            value: Resolution::LastPressed,
            title: "Last Input Priority".to_owned(),
            description: "The last activated key overrides the previous.".to_owned(),
        },
        Choice {
            value: Resolution::First,
            title: format!("Absolute Priority ({first} Key)"),
            description: format!(
                "The {first} Key will always take priority over the {second} Key."
            ),
        },
        Choice {
            value: Resolution::Second,
            title: format!("Absolute Priority ({second} Key)"),
            description: format!(
                "The {second} Key will always take priority over the {first} Key."
            ),
        },
        Choice {
            value: Resolution::Neither,
            title: "Neutral".to_owned(),
            description: "When both keys are activated, neither are registered.".to_owned(),
        },
        Choice {
            value: Resolution::Both,
            title: "No cleaning".to_owned(),
            description: "When both keys are activated, both are registered.".to_owned(),
        },
    ]
}

pub(crate) fn bottomed_out_resolutions(first: &str, second: &str) -> [Choice<Resolution>; 5] {
    [
        Choice {
            value: Resolution::Both,
            title: "Both keys".to_owned(),
            description: "When both keys are fully pressed, both will be activated.".to_owned(),
        },
        Choice {
            value: Resolution::Neither,
            title: "Neither key".to_owned(),
            description: "When both keys are fully pressed, neither is activated.".to_owned(),
        },
        Choice {
            value: Resolution::First,
            title: format!("The {first} Key"),
            description: format!("When both keys are fully pressed, only {first} is activated."),
        },
        Choice {
            value: Resolution::Second,
            title: format!("The {second} Key"),
            description: format!("When both keys are fully pressed, only {second} is activated."),
        },
        Choice {
            value: Resolution::LastPressed,
            title: "Last Input Priority".to_owned(),
            description: "When both keys are fully pressed, the one pressed last is activated."
                .to_owned(),
        },
    ]
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
        for kind in Kind::ALL {
            let behaviour = kind.behaviour(Action::Keyboard(0x04), &keys);
            assert_eq!(behaviour.as_ref().map(Kind::of), Some(kind));
        }
    }
}
