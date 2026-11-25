use openwoot_protocol::{device::Mode, gamepad::Control};

use super::system;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Controller {
    Xbox,
    Classic,
}

pub(super) const XBOX: [Control; 25] = [
    Control::ButtonA,
    Control::ButtonB,
    Control::ButtonX,
    Control::ButtonY,
    Control::Start,
    Control::Back,
    Control::Guide,
    Control::DpadUp,
    Control::DpadDown,
    Control::DpadLeft,
    Control::DpadRight,
    Control::LeftStickUp,
    Control::LeftStickDown,
    Control::LeftStickLeft,
    Control::LeftStickRight,
    Control::LeftStickPress,
    Control::RightStickUp,
    Control::RightStickDown,
    Control::RightStickLeft,
    Control::RightStickRight,
    Control::RightStickPress,
    Control::LeftBumper,
    Control::LeftTrigger,
    Control::RightBumper,
    Control::RightTrigger,
];

pub(super) const CLASSIC: [Control; 33] = [
    Control::LeftStickLeft,
    Control::LeftStickRight,
    Control::LeftStickUp,
    Control::LeftStickDown,
    Control::RxNegative,
    Control::RxPositive,
    Control::RyNegative,
    Control::RyPositive,
    Control::RightStickLeft,
    Control::RightStickRight,
    Control::RightStickUp,
    Control::RightStickDown,
    Control::LeftTrigger,
    Control::RightTrigger,
    Control::Accelerator,
    Control::Rudder,
    Control::Trigger,
    Control::ButtonA,
    Control::ButtonB,
    Control::ButtonX,
    Control::ButtonY,
    Control::Start,
    Control::Back,
    Control::DpadUp,
    Control::DpadDown,
    Control::DpadLeft,
    Control::DpadRight,
    Control::LeftBumper,
    Control::RightBumper,
    Control::HatUp,
    Control::HatDown,
    Control::HatLeft,
    Control::HatRight,
];

impl Controller {
    pub(super) const PREFERRED: Self = if system::WITHOUT_XBOX_DRIVER.is_some() {
        Self::Classic
    } else {
        Self::Xbox
    };

    pub(super) const fn of(mode: Mode) -> Option<Self> {
        match mode {
            Mode::Xinput => Some(Self::Xbox),
            Mode::Gamepad => Some(Self::Classic),
            Mode::NoGamepad => None,
        }
    }

    pub(super) const fn mode(self) -> Mode {
        match self {
            Self::Xbox => Mode::Xinput,
            Self::Classic => Mode::Gamepad,
        }
    }

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Xbox => "Xbox Controller",
            Self::Classic => "Classic Controller",
        }
    }

    pub(super) const fn palette(self) -> &'static [Control] {
        match self {
            Self::Xbox => &XBOX,
            Self::Classic => &CLASSIC,
        }
    }

    pub(super) fn sends(self, control: Control) -> bool {
        self.palette().contains(&control)
    }

    pub(super) const fn other(self) -> Self {
        match self {
            Self::Xbox => Self::Classic,
            Self::Classic => Self::Xbox,
        }
    }
}

pub(super) fn shown_as(control: Control, controller: Controller) -> Controller {
    if controller.sends(control) {
        controller
    } else {
        controller.other()
    }
}

pub(super) fn name(control: Control, controller: Controller) -> &'static str {
    match shown_as(control, controller) {
        Controller::Xbox => xbox_name(control),
        Controller::Classic => classic_name(control),
    }
}

pub(super) fn tooltip(control: Control, controller: Controller) -> String {
    let name = name(control, controller);
    if controller.sends(control) {
        name.to_owned()
    } else {
        format!("{name} · {} only", controller.other().name())
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

const fn xbox_name(control: Control) -> &'static str {
    match control {
        Control::ButtonA => "Button A",
        Control::ButtonB => "Button B",
        Control::ButtonX => "Button X",
        Control::ButtonY => "Button Y",
        Control::Start => "Start",
        Control::Back => "Select",
        Control::Guide => "Menu button",
        Control::DpadUp | Control::HatUp => "Arrow Up",
        Control::DpadDown | Control::HatDown => "Arrow Down",
        Control::DpadLeft | Control::HatLeft => "Arrow Left",
        Control::DpadRight | Control::HatRight => "Arrow Right",
        Control::LeftStickUp => "Left Joystick Up",
        Control::LeftStickDown => "Left Joystick Down",
        Control::LeftStickLeft => "Left Joystick Left",
        Control::LeftStickRight => "Left Joystick Right",
        Control::LeftStickPress => "Left Joystick Press",
        Control::RightStickUp => "Right Joystick Up",
        Control::RightStickDown => "Right Joystick Down",
        Control::RightStickLeft => "Right Joystick Left",
        Control::RightStickRight => "Right Joystick Right",
        Control::RightStickPress => "Right Joystick Press",
        Control::LeftBumper => "Left Bumper",
        Control::RightBumper => "Right Bumper",
        Control::LeftTrigger => "Left Trigger",
        Control::RightTrigger => "Right Trigger",
        Control::Accelerator
        | Control::Rudder
        | Control::Trigger
        | Control::RxNegative
        | Control::RxPositive
        | Control::RyNegative
        | Control::RyPositive => classic_name(control),
    }
}

const fn classic_name(control: Control) -> &'static str {
    match control {
        Control::LeftStickLeft => "X Axis Left (Joystick 1 Left)",
        Control::LeftStickRight => "X Axis Right (Joystick 1 Right)",
        Control::LeftStickUp => "Y Axis Up (Joystick 1 Up)",
        Control::LeftStickDown => "Y Axis Down (Joystick 1 Down)",
        Control::RxNegative => "Z Axis Forward (Joystick 2 Left)",
        Control::RxPositive => "Z Axis Backward (Joystick 2 Right)",
        Control::RyNegative => "Z Rotation Forward (Joystick 2 Up)",
        Control::RyPositive => "Z Rotation Back (Joystick 2 Down)",
        Control::RightStickLeft => "X Rotation Left",
        Control::RightStickRight => "X Rotation Right",
        Control::RightStickUp => "Y Rotation Up",
        Control::RightStickDown => "Y Rotation Down",
        Control::LeftTrigger => "Throttle",
        Control::RightTrigger => "Elevator",
        Control::Accelerator => "Accelerator",
        Control::Rudder => "Rudder",
        Control::Trigger => "Trigger",
        Control::ButtonA => "Button 1",
        Control::ButtonB => "Button 2",
        Control::ButtonX => "Button 3",
        Control::ButtonY => "Button 4",
        Control::Start => "Button 5",
        Control::Back => "Button 6",
        Control::DpadUp => "Button 7",
        Control::DpadDown => "Button 8",
        Control::DpadLeft => "Button 9",
        Control::DpadRight => "Button 10",
        Control::LeftBumper => "Button 11",
        Control::RightBumper => "Button 12",
        Control::HatUp => "Arrow Up",
        Control::HatDown => "Arrow Down",
        Control::HatLeft => "Arrow Left",
        Control::HatRight => "Arrow Right",
        Control::Guide | Control::LeftStickPress | Control::RightStickPress => xbox_name(control),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::GAMEPAD;

    #[test]
    fn every_control_is_on_one_palette_or_both() {
        for (control, short, _) in GAMEPAD {
            assert!(
                Controller::Xbox.sends(*control) || Controller::Classic.sends(*control),
                "{short}"
            );
        }
        let distinct = |palette: &[Control]| {
            palette.iter().enumerate().all(|(index, control)| {
                !palette.iter().skip(index + 1).any(|other| other == control)
            })
        };
        assert!(distinct(&XBOX));
        assert!(distinct(&CLASSIC));
    }

    #[test]
    fn the_firmware_sends_the_xbox_extras_only_as_xinput_and_the_axes_only_as_hid() {
        assert!(Controller::Xbox.sends(Control::Guide));
        assert!(!Controller::Classic.sends(Control::Guide));
        assert!(!Controller::Classic.sends(Control::LeftStickPress));
        assert!(Controller::Classic.sends(Control::HatUp));
        assert!(!Controller::Xbox.sends(Control::HatUp));
        assert!(!Controller::Xbox.sends(Control::Accelerator));
    }

    #[test]
    fn classic_names_follow_the_hid_report_and_xbox_only_controls_say_so() {
        assert_eq!(name(Control::ButtonX, Controller::Classic), "Button 3");
        assert_eq!(name(Control::ButtonX, Controller::Xbox), "Button X");
        assert_eq!(name(Control::LeftTrigger, Controller::Classic), "Throttle");
        assert_eq!(
            name(Control::RxNegative, Controller::Xbox),
            "Z Axis Forward (Joystick 2 Left)"
        );
        assert_eq!(
            tooltip(Control::Guide, Controller::Classic),
            "Menu button · Xbox Controller only"
        );
        assert_eq!(
            tooltip(Control::HatLeft, Controller::Xbox),
            "Arrow Left · Classic Controller only"
        );
    }
}
