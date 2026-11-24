use openwoot_protocol::{device::Mode, gamepad::Control};

pub(super) struct Group {
    pub(super) title: &'static str,
    pub(super) controls: &'static [Control],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Support {
    Both,
    XboxOnly,
    ClassicOnly,
}

pub(super) const GROUPS: [Group; 7] = [
    Group {
        title: "Buttons",
        controls: &[
            Control::ButtonA,
            Control::ButtonB,
            Control::ButtonX,
            Control::ButtonY,
            Control::LeftBumper,
            Control::RightBumper,
            Control::Back,
            Control::Start,
            Control::Guide,
            Control::LeftStickPress,
            Control::RightStickPress,
        ],
    },
    Group {
        title: "D-pad",
        controls: &[
            Control::DpadUp,
            Control::DpadDown,
            Control::DpadLeft,
            Control::DpadRight,
        ],
    },
    Group {
        title: "Left stick",
        controls: &[
            Control::LeftStickUp,
            Control::LeftStickDown,
            Control::LeftStickLeft,
            Control::LeftStickRight,
        ],
    },
    Group {
        title: "Right stick",
        controls: &[
            Control::RightStickUp,
            Control::RightStickDown,
            Control::RightStickLeft,
            Control::RightStickRight,
        ],
    },
    Group {
        title: "Triggers",
        controls: &[Control::LeftTrigger, Control::RightTrigger],
    },
    Group {
        title: "Axes",
        controls: &[
            Control::Accelerator,
            Control::Rudder,
            Control::Trigger,
            Control::RxNegative,
            Control::RxPositive,
            Control::RyNegative,
            Control::RyPositive,
        ],
    },
    Group {
        title: "Hat",
        controls: &[
            Control::HatUp,
            Control::HatDown,
            Control::HatLeft,
            Control::HatRight,
        ],
    },
];

pub(super) const fn support(control: Control) -> Support {
    match control {
        Control::Guide | Control::LeftStickPress | Control::RightStickPress => Support::XboxOnly,
        Control::Accelerator
        | Control::Rudder
        | Control::Trigger
        | Control::RxNegative
        | Control::RxPositive
        | Control::RyNegative
        | Control::RyPositive
        | Control::HatUp
        | Control::HatDown
        | Control::HatLeft
        | Control::HatRight => Support::ClassicOnly,
        _ => Support::Both,
    }
}

pub(super) const fn works_in(control: Control, mode: Mode) -> bool {
    match (support(control), mode) {
        (Support::Both, _)
        | (Support::XboxOnly, Mode::Xinput)
        | (Support::ClassicOnly, Mode::Gamepad) => true,
        (Support::XboxOnly | Support::ClassicOnly, _) => false,
    }
}

pub(super) const fn is_analog(control: Control) -> bool {
    matches!(
        control,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::GAMEPAD;

    #[test]
    fn every_control_appears_in_exactly_one_group() {
        for (control, short, _) in GAMEPAD {
            let appearances = GROUPS
                .iter()
                .flat_map(|group| group.controls)
                .filter(|listed| *listed == control)
                .count();
            assert_eq!(appearances, 1, "{short}");
        }
        let listed: usize = GROUPS.iter().map(|group| group.controls.len()).sum();
        assert_eq!(listed, GAMEPAD.len());
    }

    #[test]
    fn controls_only_one_controller_type_has_are_marked() {
        assert!(works_in(Control::Guide, Mode::Xinput));
        assert!(!works_in(Control::Guide, Mode::Gamepad));
        assert!(works_in(Control::HatUp, Mode::Gamepad));
        assert!(!works_in(Control::HatUp, Mode::Xinput));
        assert!(works_in(Control::ButtonA, Mode::NoGamepad));
    }
}
