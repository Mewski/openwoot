use openwoot_protocol::gamepad::Settings;

use super::{
    AxisPriority, GamepadInput, axis::Directions, combine_axis, response_curve, shape_stick,
    trigger::trigger_value,
};
use crate::{
    hid::{GAMEPAD_SLIDER_MAX, GAMEPAD_STICK_CENTER, GAMEPAD_STICK_MAX, GamepadReport},
    xinput::{self, Stick},
};

const XINPUT_SCALE_POSITIVE: f32 = i16::MAX as f32;
const XINPUT_SCALE_NEGATIVE: f32 = -(i16::MIN as f32);
const XINPUT_STICK_MIN: f32 = 1.0 / XINPUT_SCALE_POSITIVE;
const HID_SCALE: f32 = GAMEPAD_STICK_CENTER as f32;
const HID_STICK_MIN: f32 = 1.0 / HID_SCALE;
const DETECTION_CORNERS: [Stick; 4] = [
    Stick {
        x: i16::MAX,
        y: i16::MAX,
    },
    Stick {
        x: i16::MAX,
        y: i16::MIN,
    },
    Stick {
        x: i16::MIN,
        y: i16::MIN,
    },
    Stick {
        x: i16::MIN,
        y: i16::MAX,
    },
];
const DETECTION_WITHOUT_PLAYER: Stick = Stick {
    x: i16::MAX,
    y: i16::MAX,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gamepad {
    sticks: [[AxisPriority; 2]; 2],
    extra_axes: [AxisPriority; 2],
}

impl Gamepad {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sticks: [[AxisPriority::None; 2]; 2],
            extra_axes: [AxisPriority::None; 2],
        }
    }

    #[must_use]
    pub fn xinput_detection(player: Option<u8>) -> xinput::Report {
        match player.and_then(|player| DETECTION_CORNERS.get(usize::from(player))) {
            Some(&left_stick) => xinput::Report {
                left_stick,
                ..xinput::Report::default()
            },
            None => xinput::Report {
                right_stick: DETECTION_WITHOUT_PLAYER,
                ..xinput::Report::default()
            },
        }
    }

    #[must_use]
    pub const fn hid_detection() -> GamepadReport {
        GamepadReport {
            x: GAMEPAD_STICK_MAX,
            y: GAMEPAD_STICK_MAX,
            ..GamepadReport::new()
        }
    }

    #[must_use]
    pub const fn priorities(&self) -> [AxisPriority; 6] {
        let [[x, y], [z, rz]] = self.sticks;
        let [rx, ry] = self.extra_axes;
        [x, y, z, rz, rx, ry]
    }

    pub fn xinput_report(
        &mut self,
        input: &GamepadInput,
        settings: Option<&Settings>,
    ) -> xinput::Report {
        if !input.has_input() {
            *self = Self::new();
            return xinput::Report::default();
        }
        let [left, right] = &mut self.sticks;
        let trigger = |travel| trigger_value(travel, u8::MAX.into(), settings) as u8;
        xinput::Report {
            buttons: input.xinput_buttons(),
            left_trigger: trigger(input.left_trigger),
            right_trigger: trigger(input.right_trigger),
            left_stick: xinput_stick(input.left_stick, left, true, settings),
            right_stick: xinput_stick(input.right_stick, right, false, settings),
        }
    }

    pub fn hid_report(
        &mut self,
        input: &mut GamepadInput,
        settings: Option<&Settings>,
    ) -> GamepadReport {
        if !input.has_input() {
            *self = Self::new();
            return GamepadReport::new();
        }
        let [first, second] = &mut self.sticks;
        let [extra_x, extra_y] = &mut self.extra_axes;
        let (x, y) = hid_stick(input.left_stick, first, true, settings);
        let (z, rz) = hid_stick(input.rotation, second, false, settings);
        let slider = |travel| trigger_value(travel, GAMEPAD_SLIDER_MAX, settings);
        GamepadReport {
            buttons: input.buttons(),
            x,
            y,
            z,
            rz,
            rx: hid_extra_axis(
                input.right_stick.right,
                input.right_stick.left,
                extra_x,
                settings,
            ),
            ry: hid_extra_axis(
                input.right_stick.down,
                input.right_stick.up,
                extra_y,
                settings,
            ),
            throttle: slider(input.left_trigger),
            elevator: slider(input.right_trigger),
            accelerator: slider(input.accelerator),
            rudder: slider(input.rudder),
            trigger: slider(input.trigger),
            hat: input.resolve_hat(),
        }
    }
}

impl Default for Gamepad {
    fn default() -> Self {
        Self::new()
    }
}

fn xinput_stick(
    directions: Directions,
    priorities: &mut [AxisPriority; 2],
    left_stick: bool,
    settings: Option<&Settings>,
) -> Stick {
    let (x, y) = directions.combine(priorities, settings);
    let (x, y) = shape_stick(x, y, left_stick, XINPUT_STICK_MIN, settings);
    Stick {
        x: xinput_axis(x),
        y: xinput_axis(-y),
    }
}

fn xinput_axis(value: f32) -> i16 {
    let scale = if value < 0.0 {
        XINPUT_SCALE_NEGATIVE
    } else {
        XINPUT_SCALE_POSITIVE
    };
    (value * scale) as i32 as i16
}

fn hid_stick(
    directions: Directions,
    priorities: &mut [AxisPriority; 2],
    left_stick: bool,
    settings: Option<&Settings>,
) -> (u16, u16) {
    let (x, y) = directions.combine(priorities, settings);
    let (x, y) = shape_stick(x, y, left_stick, HID_STICK_MIN, settings);
    (hid_axis(x), hid_axis(y))
}

fn hid_extra_axis(
    positive: u16,
    negative: u16,
    priority: &mut AxisPriority,
    settings: Option<&Settings>,
) -> u16 {
    let value = combine_axis(positive, negative, priority, settings);
    if value < 0.0 {
        hid_axis(-response_curve(-value, settings))
    } else if value == 0.0 {
        hid_axis(0.0)
    } else {
        hid_axis(response_curve(value, settings))
    }
}

fn hid_axis(value: f32) -> u16 {
    ((value * HID_SCALE) as i32).wrapping_add(i32::from(GAMEPAD_STICK_CENTER)) as u16
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::gamepad::{Control, CurvePoint, Normalization};

    use super::*;
    use crate::{hid::Hat, usb::Mode, xinput::Buttons};

    const SINGLE_DIRECTION: Settings = Settings {
        curve: [
            CurvePoint {
                travel: 0,
                output: 0,
            },
            CurvePoint {
                travel: 85,
                output: 85,
            },
            CurvePoint {
                travel: 170,
                output: 170,
            },
            CurvePoint {
                travel: 255,
                output: 255,
            },
        ],
        normalization: Normalization::Circular,
        single_direction_axes: true,
        left_stick_up_bias: None,
    };

    fn applied(mode: Mode, bindings: &[(Control, u16, bool)]) -> GamepadInput {
        let mut input = GamepadInput::new();
        for &(control, travel, pressed) in bindings {
            input.apply(mode, control, travel, pressed);
        }
        input
    }

    #[test]
    fn without_input_the_reports_rest() {
        let mut gamepad = Gamepad::new();
        let mut input = applied(Mode::Gamepad, &[(Control::LeftStickUp, 0, false)]);
        assert_eq!(
            gamepad.xinput_report(&input, None),
            xinput::Report::default()
        );
        assert_eq!(gamepad.hid_report(&mut input, None), GamepadReport::new());
    }

    #[test]
    fn xinput_sticks_reach_the_ends_of_their_range() {
        let mut gamepad = Gamepad::new();
        let input = applied(
            Mode::Xinput,
            &[
                (Control::LeftStickRight, 4095, true),
                (Control::RightStickUp, 4095, true),
                (Control::RightStickLeft, 4095, true),
                (Control::LeftTrigger, 4095, true),
                (Control::ButtonY, 0, true),
            ],
        );
        let report = gamepad.xinput_report(&input, None);
        assert_eq!(report.buttons, Buttons::BUTTON_Y);
        assert_eq!((report.left_trigger, report.right_trigger), (255, 0));
        assert_eq!(report.left_stick, Stick { x: 32_767, y: 0 });
        assert_eq!(
            report.right_stick,
            Stick {
                x: -23_170,
                y: 23_169
            }
        );
    }

    #[test]
    fn hid_axes_are_centred_and_scaled() {
        let mut gamepad = Gamepad::new();
        let mut input = applied(
            Mode::Gamepad,
            &[
                (Control::LeftStickLeft, 4095, true),
                (Control::RxPositive, 4095, true),
                (Control::RightStickDown, 4095, true),
                (Control::Trigger, 4095, true),
                (Control::ButtonB, 0, true),
                (Control::HatRight, 0, true),
            ],
        );
        assert_eq!(
            gamepad.hid_report(&mut input, None),
            GamepadReport {
                buttons: 0b10,
                x: 0,
                z: GAMEPAD_STICK_MAX,
                ry: GAMEPAD_STICK_MAX,
                trigger: GAMEPAD_SLIDER_MAX,
                hat: Hat::Right,
                ..GamepadReport::new()
            }
        );
    }

    #[test]
    fn the_axis_priorities_persist_between_passes() {
        let mut gamepad = Gamepad::new();
        let deeper_left = applied(
            Mode::Xinput,
            &[
                (Control::LeftStickLeft, 2000, true),
                (Control::LeftStickRight, 1000, true),
            ],
        );
        let report = gamepad.xinput_report(&deeper_left, Some(&SINGLE_DIRECTION));
        assert!(report.left_stick.x < 0);
        assert_eq!(
            gamepad.priorities(),
            [
                AxisPriority::Negative,
                AxisPriority::None,
                AxisPriority::None,
                AxisPriority::None,
                AxisPriority::None,
                AxisPriority::None,
            ]
        );
        let both_bottomed_out = applied(
            Mode::Xinput,
            &[
                (Control::LeftStickLeft, 4095, true),
                (Control::LeftStickRight, 4095, true),
            ],
        );
        let report = gamepad.xinput_report(&both_bottomed_out, Some(&SINGLE_DIRECTION));
        assert_eq!(report.left_stick.x, 32_767);
        assert_eq!(gamepad.priorities()[0], AxisPriority::PositiveTookOver);
    }

    #[test]
    fn releasing_every_control_forgets_the_axis_priorities() {
        let mut gamepad = Gamepad::new();
        let right = applied(Mode::Xinput, &[(Control::LeftStickRight, 2000, true)]);
        gamepad.xinput_report(&right, Some(&SINGLE_DIRECTION));
        assert_eq!(gamepad.priorities()[0], AxisPriority::Positive);
        gamepad.xinput_report(&GamepadInput::new(), Some(&SINGLE_DIRECTION));
        assert_eq!(gamepad.priorities()[0], AxisPriority::None);
        let mut left = applied(Mode::Gamepad, &[(Control::LeftStickLeft, 2000, true)]);
        gamepad.hid_report(&mut left, Some(&SINGLE_DIRECTION));
        assert_eq!(gamepad.priorities()[0], AxisPriority::Negative);
        gamepad.hid_report(&mut GamepadInput::new(), Some(&SINGLE_DIRECTION));
        assert_eq!(gamepad.priorities()[0], AxisPriority::None);
    }
}
