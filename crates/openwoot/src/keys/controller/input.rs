use openwoot_protocol::gamepad::Control;

use super::{axis::Directions, gamepad_code};
use crate::{hid::Hat, usb::Mode, xinput::Buttons};

const FIRST_BUTTON: u8 = gamepad_code(Control::ButtonA);
const FIRST_HAT: u8 = gamepad_code(Control::HatUp);
const HAT_UP: u8 = 1 << 0;
const HAT_DOWN: u8 = 1 << 1;
const HAT_LEFT: u8 = 1 << 2;
const HAT_RIGHT: u8 = 1 << 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GamepadInput {
    active: bool,
    xinput_buttons: Option<Buttons>,
    buttons: u16,
    hat: u8,
    pub(super) left_stick: Directions,
    pub(super) right_stick: Directions,
    pub(super) rotation: Directions,
    pub(super) left_trigger: u16,
    pub(super) right_trigger: u16,
    pub(super) accelerator: u16,
    pub(super) rudder: u16,
    pub(super) trigger: u16,
}

impl GamepadInput {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            xinput_buttons: None,
            buttons: 0,
            hat: 0,
            left_stick: Directions::new(),
            right_stick: Directions::new(),
            rotation: Directions::new(),
            left_trigger: 0,
            right_trigger: 0,
            accelerator: 0,
            rudder: 0,
            trigger: 0,
        }
    }

    pub fn apply(&mut self, mode: Mode, control: Control, travel: u16, pressed: bool) {
        let known = match mode {
            Mode::Xinput => self.apply_xinput(control, travel, pressed),
            Mode::Gamepad => self.apply_hid(control, travel, pressed),
            Mode::NoGamepad => true,
        };
        if known && (pressed || travel != 0) {
            self.active = true;
        }
    }

    #[must_use]
    pub const fn has_input(&self) -> bool {
        self.active
    }

    pub(super) fn xinput_buttons(&self) -> Buttons {
        self.xinput_buttons.unwrap_or_default()
    }

    pub(super) const fn buttons(&self) -> u16 {
        self.buttons
    }

    pub(super) fn resolve_hat(&mut self) -> Hat {
        if self.hat & (HAT_UP | HAT_DOWN) == HAT_UP | HAT_DOWN {
            self.hat &= !(HAT_UP | HAT_DOWN);
        }
        if self.hat & (HAT_LEFT | HAT_RIGHT) == HAT_LEFT | HAT_RIGHT {
            self.hat &= !(HAT_LEFT | HAT_RIGHT);
        }
        let held = |direction: u8| self.hat & direction != 0;
        match (
            held(HAT_UP),
            held(HAT_DOWN),
            held(HAT_LEFT),
            held(HAT_RIGHT),
        ) {
            (true, _, true, _) => Hat::UpLeft,
            (true, _, _, true) => Hat::UpRight,
            (true, ..) => Hat::Up,
            (_, true, true, _) => Hat::DownLeft,
            (_, true, _, true) => Hat::DownRight,
            (_, true, ..) => Hat::Down,
            (.., true, _) => Hat::Left,
            (.., true) => Hat::Right,
            _ => Hat::Centered,
        }
    }

    fn apply_xinput(&mut self, control: Control, travel: u16, pressed: bool) -> bool {
        let button = match control {
            Control::ButtonA => Buttons::BUTTON_A,
            Control::ButtonB => Buttons::BUTTON_B,
            Control::ButtonX => Buttons::BUTTON_X,
            Control::ButtonY => Buttons::BUTTON_Y,
            Control::Start => Buttons::START,
            Control::Back => Buttons::BACK,
            Control::DpadUp => Buttons::DPAD_UP,
            Control::DpadDown => Buttons::DPAD_DOWN,
            Control::DpadLeft => Buttons::DPAD_LEFT,
            Control::DpadRight => Buttons::DPAD_RIGHT,
            Control::LeftBumper => Buttons::LEFT_SHOULDER,
            Control::RightBumper => Buttons::RIGHT_SHOULDER,
            Control::Guide => Buttons::GUIDE,
            Control::LeftStickPress => Buttons::LEFT_THUMB,
            Control::RightStickPress => Buttons::RIGHT_THUMB,
            Control::LeftStickUp
            | Control::LeftStickDown
            | Control::LeftStickLeft
            | Control::LeftStickRight
            | Control::RightStickUp
            | Control::RightStickDown
            | Control::RightStickLeft
            | Control::RightStickRight
            | Control::LeftTrigger
            | Control::RightTrigger => return self.follow(control, travel),
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
            | Control::HatRight => return false,
        };
        if pressed {
            self.xinput_buttons = Some(
                self.xinput_buttons
                    .map_or(button, |held| held.union(button)),
            );
        }
        true
    }

    fn apply_hid(&mut self, control: Control, travel: u16, pressed: bool) -> bool {
        match control {
            Control::Guide | Control::LeftStickPress | Control::RightStickPress => false,
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
            | Control::RightBumper => {
                if pressed {
                    self.buttons |= 1 << (gamepad_code(control) - FIRST_BUTTON);
                }
                true
            }
            Control::HatUp | Control::HatDown | Control::HatLeft | Control::HatRight => {
                if pressed {
                    self.hat |= 1 << (gamepad_code(control) - FIRST_HAT);
                }
                true
            }
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
            | Control::RyPositive => self.follow(control, travel),
        }
    }

    fn follow(&mut self, control: Control, travel: u16) -> bool {
        let Some(slot) = self.travel_mut(control) else {
            return false;
        };
        *slot = (*slot).max(travel);
        true
    }

    fn travel_mut(&mut self, control: Control) -> Option<&mut u16> {
        Some(match control {
            Control::LeftStickUp => &mut self.left_stick.up,
            Control::LeftStickDown => &mut self.left_stick.down,
            Control::LeftStickLeft => &mut self.left_stick.left,
            Control::LeftStickRight => &mut self.left_stick.right,
            Control::RightStickUp => &mut self.right_stick.up,
            Control::RightStickDown => &mut self.right_stick.down,
            Control::RightStickLeft => &mut self.right_stick.left,
            Control::RightStickRight => &mut self.right_stick.right,
            Control::LeftTrigger => &mut self.left_trigger,
            Control::RightTrigger => &mut self.right_trigger,
            Control::Accelerator => &mut self.accelerator,
            Control::Rudder => &mut self.rudder,
            Control::Trigger => &mut self.trigger,
            Control::RxNegative => &mut self.rotation.left,
            Control::RxPositive => &mut self.rotation.right,
            Control::RyNegative => &mut self.rotation.up,
            Control::RyPositive => &mut self.rotation.down,
            _ => return None,
        })
    }
}

impl Default for GamepadInput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn applied(mode: Mode, bindings: &[(Control, u16, bool)]) -> GamepadInput {
        let mut input = GamepadInput::new();
        for &(control, travel, pressed) in bindings {
            input.apply(mode, control, travel, pressed);
        }
        input
    }

    #[test]
    fn buttons_are_ored_and_directions_keep_the_deepest_travel() {
        let input = applied(
            Mode::Xinput,
            &[
                (Control::ButtonA, 3000, true),
                (Control::ButtonA, 100, false),
                (Control::Guide, 4095, true),
                (Control::LeftStickUp, 1000, false),
                (Control::LeftStickUp, 500, false),
                (Control::RightTrigger, 2000, true),
            ],
        );
        assert!(input.has_input());
        assert_eq!(
            input.xinput_buttons(),
            Buttons::BUTTON_A.union(Buttons::GUIDE)
        );
        assert_eq!(input.left_stick.up, 1000);
        assert_eq!(input.right_trigger, 2000);
    }

    #[test]
    fn controls_unknown_to_the_mode_are_ignored() {
        let xinput = applied(
            Mode::Xinput,
            &[
                (Control::Accelerator, 4095, true),
                (Control::HatUp, 0, true),
            ],
        );
        assert!(!xinput.has_input());
        assert_eq!(xinput.accelerator, 0);
        let hid = applied(
            Mode::Gamepad,
            &[
                (Control::Guide, 4095, true),
                (Control::LeftStickPress, 0, true),
            ],
        );
        assert!(!hid.has_input());
        assert_eq!(hid.buttons(), 0);
    }

    #[test]
    fn a_released_control_at_rest_gives_no_input() {
        let mut input = applied(
            Mode::Gamepad,
            &[
                (Control::ButtonB, 0, false),
                (Control::LeftTrigger, 0, false),
            ],
        );
        assert!(!input.has_input());
        input.apply(Mode::Gamepad, Control::ButtonB, 10, false);
        assert!(input.has_input());
        assert_eq!(input.buttons(), 0);
    }

    #[test]
    fn without_a_gamepad_interface_every_control_only_counts_as_input() {
        let input = applied(
            Mode::NoGamepad,
            &[
                (Control::ButtonA, 0, true),
                (Control::LeftStickUp, 4095, false),
            ],
        );
        assert!(input.has_input());
        assert_eq!(input.xinput_buttons(), Buttons::default());
        assert_eq!(input.left_stick.up, 0);
    }

    #[test]
    fn hid_buttons_follow_the_code_order() {
        let input = applied(
            Mode::Gamepad,
            &[
                (Control::ButtonA, 0, true),
                (Control::DpadUp, 0, true),
                (Control::RightBumper, 0, true),
            ],
        );
        assert_eq!(input.buttons(), 0b1000_0100_0001);
    }

    #[test]
    fn opposite_hat_directions_cancel_each_other() {
        let mut input = applied(
            Mode::Gamepad,
            &[
                (Control::HatUp, 0, true),
                (Control::HatDown, 0, true),
                (Control::HatLeft, 0, true),
            ],
        );
        assert_eq!(input.resolve_hat(), Hat::Left);
        assert_eq!(input.hat, HAT_LEFT);
        let mut diagonal = applied(
            Mode::Gamepad,
            &[(Control::HatDown, 0, true), (Control::HatRight, 0, true)],
        );
        assert_eq!(diagonal.resolve_hat(), Hat::DownRight);
        let mut every = applied(
            Mode::Gamepad,
            &[
                (Control::HatUp, 0, true),
                (Control::HatDown, 0, true),
                (Control::HatLeft, 0, true),
                (Control::HatRight, 0, true),
            ],
        );
        assert_eq!(every.resolve_hat(), Hat::Centered);
    }
}
