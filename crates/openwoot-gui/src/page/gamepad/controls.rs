use openwoot_protocol::{device::Mode, gamepad::Control};

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
    pub(super) const PREFERRED: Self = if cfg!(target_os = "macos") {
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
            Self::Xbox => "Xbox controller",
            Self::Classic => "Classic controller",
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
        Control::DpadUp | Control::HatUp => "Arrow up",
        Control::DpadDown | Control::HatDown => "Arrow down",
        Control::DpadLeft | Control::HatLeft => "Arrow left",
        Control::DpadRight | Control::HatRight => "Arrow right",
        Control::LeftStickUp => "Left joystick up",
        Control::LeftStickDown => "Left joystick down",
        Control::LeftStickLeft => "Left joystick left",
        Control::LeftStickRight => "Left joystick right",
        Control::LeftStickPress => "Left joystick press",
        Control::RightStickUp => "Right joystick up",
        Control::RightStickDown => "Right joystick down",
        Control::RightStickLeft => "Right joystick left",
        Control::RightStickRight => "Right joystick right",
        Control::RightStickPress => "Right joystick press",
        Control::LeftBumper => "Left bumper",
        Control::RightBumper => "Right bumper",
        Control::LeftTrigger => "Left trigger",
        Control::RightTrigger => "Right trigger",
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
        Control::LeftStickLeft => "X axis left (joystick 1 left)",
        Control::LeftStickRight => "X axis right (joystick 1 right)",
        Control::LeftStickUp => "Y axis up (joystick 1 up)",
        Control::LeftStickDown => "Y axis down (joystick 1 down)",
        Control::RxNegative => "Z axis forward (joystick 2 left)",
        Control::RxPositive => "Z axis backward (joystick 2 right)",
        Control::RyNegative => "Z rotation forward (joystick 2 up)",
        Control::RyPositive => "Z rotation back (joystick 2 down)",
        Control::RightStickLeft => "X rotation left",
        Control::RightStickRight => "X rotation right",
        Control::RightStickUp => "Y rotation up",
        Control::RightStickDown => "Y rotation down",
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
        Control::HatUp => "Arrow up",
        Control::HatDown => "Arrow down",
        Control::HatLeft => "Arrow left",
        Control::HatRight => "Arrow right",
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
            "Z axis forward (joystick 2 left)"
        );
        assert_eq!(
            tooltip(Control::Guide, Controller::Classic),
            "Menu button · Xbox controller only"
        );
        assert_eq!(
            tooltip(Control::HatLeft, Controller::Xbox),
            "Arrow left · Classic controller only"
        );
    }
}
