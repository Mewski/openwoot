use openwoot_protocol::gamepad::Control;

#[must_use]
pub(crate) const fn gamepad_code(control: Control) -> u8 {
    match control {
        Control::ButtonA => 120,
        Control::ButtonB => 121,
        Control::ButtonX => 122,
        Control::ButtonY => 123,
        Control::Start => 124,
        Control::Back => 125,
        Control::DpadUp => 126,
        Control::DpadDown => 127,
        Control::DpadLeft => 128,
        Control::DpadRight => 129,
        Control::LeftBumper => 130,
        Control::RightBumper => 131,
        Control::Guide => 132,
        Control::LeftStickUp => 133,
        Control::LeftStickDown => 134,
        Control::LeftStickLeft => 135,
        Control::LeftStickRight => 136,
        Control::RightStickUp => 137,
        Control::RightStickDown => 138,
        Control::RightStickLeft => 139,
        Control::RightStickRight => 140,
        Control::LeftStickPress => 141,
        Control::RightStickPress => 142,
        Control::LeftTrigger => 143,
        Control::RightTrigger => 144,
        Control::Accelerator => 145,
        Control::Rudder => 146,
        Control::Trigger => 147,
        Control::RxNegative => 148,
        Control::RxPositive => 149,
        Control::RyNegative => 150,
        Control::RyPositive => 151,
        Control::HatUp => 152,
        Control::HatDown => 153,
        Control::HatLeft => 154,
        Control::HatRight => 155,
    }
}

#[must_use]
pub const fn follows_travel(control: Control) -> bool {
    !matches!(
        control,
        Control::ButtonA
            | Control::ButtonB
            | Control::ButtonX
            | Control::ButtonY
            | Control::Start
            | Control::Back
            | Control::DpadUp
            | Control::DpadDown
            | Control::DpadLeft
            | Control::DpadRight
            | Control::LeftBumper
            | Control::RightBumper
            | Control::Guide
            | Control::LeftStickPress
            | Control::RightStickPress
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gamepad_control(code: u8) -> Option<Control> {
        Some(match code {
            120 => Control::ButtonA,
            121 => Control::ButtonB,
            122 => Control::ButtonX,
            123 => Control::ButtonY,
            124 => Control::Start,
            125 => Control::Back,
            126 => Control::DpadUp,
            127 => Control::DpadDown,
            128 => Control::DpadLeft,
            129 => Control::DpadRight,
            130 => Control::LeftBumper,
            131 => Control::RightBumper,
            132 => Control::Guide,
            133 => Control::LeftStickUp,
            134 => Control::LeftStickDown,
            135 => Control::LeftStickLeft,
            136 => Control::LeftStickRight,
            137 => Control::RightStickUp,
            138 => Control::RightStickDown,
            139 => Control::RightStickLeft,
            140 => Control::RightStickRight,
            141 => Control::LeftStickPress,
            142 => Control::RightStickPress,
            143 => Control::LeftTrigger,
            144 => Control::RightTrigger,
            145 => Control::Accelerator,
            146 => Control::Rudder,
            147 => Control::Trigger,
            148 => Control::RxNegative,
            149 => Control::RxPositive,
            150 => Control::RyNegative,
            151 => Control::RyPositive,
            152 => Control::HatUp,
            153 => Control::HatDown,
            154 => Control::HatLeft,
            155 => Control::HatRight,
            _ => return None,
        })
    }

    #[test]
    fn every_code_from_120_to_155_names_one_control() {
        for code in 120..=155 {
            let control = gamepad_control(code).unwrap();
            assert_eq!(gamepad_code(control), code);
        }
        for code in (0..120).chain(156..=u8::MAX) {
            assert_eq!(gamepad_control(code), None);
        }
    }

    #[test]
    fn sticks_triggers_axes_and_hat_follow_the_travel() {
        let following = (120..=155).filter(|code| follows_travel(gamepad_control(*code).unwrap()));
        assert!(following.eq((133..=140).chain(143..=155)));
    }
}
