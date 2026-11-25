use std::cmp::Ordering;

use openwoot_protocol::gamepad::{Control, Normalization, Settings};

use super::{
    controls::{self, Controller},
    curve::{self, FULL_TRAVEL},
};

const BOTTOM_OUT_LIMIT: u16 = 4013;
const XINPUT_STICK_MIN: f32 = 1.0 / 32767.0;
const HID_STICK_MIN: f32 = 1.0 / 16383.0;
const EVEN_WEIGHT: f32 = 0.5;
const PERCENT: f32 = 100.0;
const STRETCH_START: f32 = 0.965;
const STRETCH_END: f32 = 0.992;
const STRETCH_SLOPE: f32 = 1.296_296_8;
const STRETCH_OFFSET: f32 = -0.285_926_4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Priorities {
    first: [Priority; 2],
    second: [Priority; 2],
    extra: [Priority; 2],
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Stick {
    pub(super) x: f32,
    pub(super) y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Axis {
    pub(super) control: Control,
    pub(super) value: f32,
    pub(super) centred: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Output {
    pub(super) sticks: [Stick; 2],
    pub(super) axes: Vec<Axis>,
    pub(super) pressed: Vec<Control>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Priority {
    #[default]
    None,
    Positive,
    Negative,
    PositiveTookOver,
    NegativeTookOver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Positive,
    Negative,
}

#[derive(Debug, Clone, Copy, Default)]
struct Directions {
    up: u16,
    down: u16,
    left: u16,
    right: u16,
}

#[derive(Debug, Clone, Copy, Default)]
struct Held {
    left_stick: Directions,
    right_stick: Directions,
    rotation: Directions,
    left_trigger: u16,
    right_trigger: u16,
    accelerator: u16,
    rudder: u16,
    trigger: u16,
}

pub(super) fn compute(
    controller: Controller,
    keys: impl IntoIterator<Item = (Control, u16, bool)>,
    settings: Option<&Settings>,
    priorities: &mut Priorities,
) -> Output {
    let mut held = Held::default();
    let mut pressed: Vec<Control> = Vec::new();
    let mut active = false;
    for (control, travel, down) in keys {
        if !controller.sends(control) {
            continue;
        }
        if controls::is_analog(control) {
            if let Some(slot) = held.slot(control) {
                *slot = (*slot).max(travel);
            }
        } else if down && !pressed.contains(&control) {
            pressed.push(control);
        }
        active |= down || travel != 0;
    }
    if !active {
        *priorities = Priorities::default();
        return rest(controller);
    }
    let held_buttons = pressed.clone();
    pressed.retain(|control| !cancelled(*control, &held_buttons));
    let palette = controller.palette();
    pressed.sort_by_key(|control| palette.iter().position(|listed| listed == control));
    match controller {
        Controller::Xbox => Output {
            sticks: [
                stick(
                    held.left_stick,
                    &mut priorities.first,
                    true,
                    XINPUT_STICK_MIN,
                    settings,
                ),
                stick(
                    held.right_stick,
                    &mut priorities.second,
                    false,
                    XINPUT_STICK_MIN,
                    settings,
                ),
            ],
            axes: vec![
                slider(Control::LeftTrigger, held.left_trigger, settings),
                slider(Control::RightTrigger, held.right_trigger, settings),
            ],
            pressed,
        },
        Controller::Classic => {
            let [rotation_x, rotation_y] = &mut priorities.extra;
            let right = held.right_stick;
            Output {
                sticks: [
                    stick(
                        held.left_stick,
                        &mut priorities.first,
                        true,
                        HID_STICK_MIN,
                        settings,
                    ),
                    stick(
                        held.rotation,
                        &mut priorities.second,
                        false,
                        HID_STICK_MIN,
                        settings,
                    ),
                ],
                axes: vec![
                    centred(
                        Control::RightStickRight,
                        extra_axis(right.right, right.left, rotation_x, settings),
                    ),
                    centred(
                        Control::RightStickDown,
                        extra_axis(right.down, right.up, rotation_y, settings),
                    ),
                    slider(Control::LeftTrigger, held.left_trigger, settings),
                    slider(Control::RightTrigger, held.right_trigger, settings),
                    slider(Control::Accelerator, held.accelerator, settings),
                    slider(Control::Rudder, held.rudder, settings),
                    slider(Control::Trigger, held.trigger, settings),
                ],
                pressed,
            }
        }
    }
}

pub(super) fn rest(controller: Controller) -> Output {
    let still = |control| Axis {
        control,
        value: 0.0,
        centred: false,
    };
    let axes = match controller {
        Controller::Xbox => vec![still(Control::LeftTrigger), still(Control::RightTrigger)],
        Controller::Classic => vec![
            centred(Control::RightStickRight, 0.0),
            centred(Control::RightStickDown, 0.0),
            still(Control::LeftTrigger),
            still(Control::RightTrigger),
            still(Control::Accelerator),
            still(Control::Rudder),
            still(Control::Trigger),
        ],
    };
    Output {
        sticks: [Stick::default(); 2],
        axes,
        pressed: Vec::new(),
    }
}

impl Held {
    fn slot(&mut self, control: Control) -> Option<&mut u16> {
        Some(match control {
            Control::LeftStickUp => &mut self.left_stick.up,
            Control::LeftStickDown => &mut self.left_stick.down,
            Control::LeftStickLeft => &mut self.left_stick.left,
            Control::LeftStickRight => &mut self.left_stick.right,
            Control::RightStickUp => &mut self.right_stick.up,
            Control::RightStickDown => &mut self.right_stick.down,
            Control::RightStickLeft => &mut self.right_stick.left,
            Control::RightStickRight => &mut self.right_stick.right,
            Control::RxNegative => &mut self.rotation.left,
            Control::RxPositive => &mut self.rotation.right,
            Control::RyNegative => &mut self.rotation.up,
            Control::RyPositive => &mut self.rotation.down,
            Control::LeftTrigger => &mut self.left_trigger,
            Control::RightTrigger => &mut self.right_trigger,
            Control::Accelerator => &mut self.accelerator,
            Control::Rudder => &mut self.rudder,
            Control::Trigger => &mut self.trigger,
            _ => return None,
        })
    }
}

impl Priority {
    fn resolve(&mut self, positive: u16, negative: u16) -> Option<Side> {
        if positive == 0 && negative == 0 {
            *self = Self::None;
            return None;
        }
        match leader(positive, negative) {
            Some(Side::Positive) => {
                *self = Self::Positive;
                Some(Side::Positive)
            }
            Some(Side::Negative) => {
                *self = Self::Negative;
                Some(Side::Negative)
            }
            None => self.take_over(),
        }
    }

    fn take_over(&mut self) -> Option<Side> {
        match self {
            Self::None => None,
            Self::Positive => {
                *self = Self::NegativeTookOver;
                Some(Side::Negative)
            }
            Self::Negative => {
                *self = Self::PositiveTookOver;
                Some(Side::Positive)
            }
            Self::PositiveTookOver => Some(Side::Positive),
            Self::NegativeTookOver => Some(Side::Negative),
        }
    }
}

const fn centred(control: Control, value: f32) -> Axis {
    Axis {
        control,
        value,
        centred: true,
    }
}

fn slider(control: Control, travel: u16, settings: Option<&Settings>) -> Axis {
    Axis {
        control,
        value: trigger_value(travel, settings),
        centred: false,
    }
}

fn cancelled(control: Control, pressed: &[Control]) -> bool {
    let opposite = match control {
        Control::HatUp => Control::HatDown,
        Control::HatDown => Control::HatUp,
        Control::HatLeft => Control::HatRight,
        Control::HatRight => Control::HatLeft,
        _ => return false,
    };
    pressed.contains(&opposite)
}

fn stick(
    directions: Directions,
    [x, y]: &mut [Priority; 2],
    left_stick: bool,
    min_magnitude: f32,
    settings: Option<&Settings>,
) -> Stick {
    let across = combine(directions.right, directions.left, x, settings);
    let down = combine(directions.down, directions.up, y, settings);
    shape(across, down, left_stick, min_magnitude, settings)
}

fn shape(
    x: f32,
    y: f32,
    left_stick: bool,
    min_magnitude: f32,
    settings: Option<&Settings>,
) -> Stick {
    let largest = x.abs().max(y.abs());
    if largest < min_magnitude || largest <= 0.0 {
        return Stick::default();
    }
    let magnitude = respond(largest, settings);
    let up_bias = settings
        .and_then(|settings| settings.left_stick_up_bias.active())
        .filter(|_| left_stick && y < 0.0);
    let (weight_x, weight_y) = up_bias.map_or((EVEN_WEIGHT, EVEN_WEIGHT), |percent| {
        let weight_x = f32::from(percent) / PERCENT;
        (weight_x, 1.0 - weight_x)
    });
    let weighted_x = x * weight_x;
    let weighted_y = y * weight_y;
    let length = match settings.map(|settings| settings.normalization) {
        Some(Normalization::Square) => weighted_x.abs().max(weighted_y.abs()),
        _ => weighted_x.hypot(weighted_y),
    };
    if length <= 0.0 {
        return Stick::default();
    }
    Stick {
        x: weighted_x / length * magnitude,
        y: weighted_y / length * magnitude,
    }
}

fn combine(
    positive: u16,
    negative: u16,
    priority: &mut Priority,
    settings: Option<&Settings>,
) -> f32 {
    let positive_level = level(positive, settings);
    let negative_level = level(negative, settings);
    if !settings.is_some_and(|settings| settings.single_direction_axes) {
        return positive_level - negative_level;
    }
    match priority.resolve(positive, negative) {
        Some(Side::Positive) => positive_level,
        Some(Side::Negative) => -negative_level,
        None => 0.0,
    }
}

fn extra_axis(
    positive: u16,
    negative: u16,
    priority: &mut Priority,
    settings: Option<&Settings>,
) -> f32 {
    let value = combine(positive, negative, priority, settings);
    if value < 0.0 {
        -respond(-value, settings)
    } else if value > 0.0 {
        respond(value, settings)
    } else {
        0.0
    }
}

fn trigger_value(travel: u16, settings: Option<&Settings>) -> f32 {
    let level = level(travel, settings);
    if level <= 0.0 {
        return 0.0;
    }
    let level = respond(level, settings);
    if level >= STRETCH_END {
        1.0
    } else if level > STRETCH_START {
        level.mul_add(STRETCH_SLOPE, STRETCH_OFFSET)
    } else {
        level
    }
}

fn level(travel: u16, settings: Option<&Settings>) -> f32 {
    if travel == 0 {
        return 0.0;
    }
    let level = (f32::from(travel) / FULL_TRAVEL).min(1.0);
    settings.map_or(level, |settings| curve::normalize(settings.curve, level))
}

fn respond(level: f32, settings: Option<&Settings>) -> f32 {
    settings.map_or(level, |settings| curve::respond(settings.curve, level))
}

fn leader(positive: u16, negative: u16) -> Option<Side> {
    match (positive > BOTTOM_OUT_LIMIT, negative > BOTTOM_OUT_LIMIT) {
        (true, true) => None,
        (true, false) => Some(Side::Positive),
        (false, true) => Some(Side::Negative),
        (false, false) => match positive.cmp(&negative) {
            Ordering::Greater => Some(Side::Positive),
            Ordering::Less => Some(Side::Negative),
            Ordering::Equal => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use openwoot_protocol::gamepad::{CurvePoint, StickBias};

    use super::*;

    fn settings(normalization: Normalization, snappy: bool, bias: Option<u8>) -> Settings {
        Settings {
            curve: [(0, 0), (85, 85), (170, 170), (255, 255)]
                .map(|(travel, output)| CurvePoint { travel, output }),
            normalization,
            single_direction_axes: snappy,
            left_stick_up_bias: bias.map_or(StickBias::DISABLED, |percent| StickBias {
                enabled: true,
                percent,
            }),
        }
    }

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 1e-3
    }

    #[test]
    fn a_full_key_moves_the_stick_to_the_edge_and_two_keys_to_the_diagonal() {
        let circular = settings(Normalization::Circular, false, None);
        let mut priorities = Priorities::default();
        let up = compute(
            Controller::Xbox,
            [(Control::LeftStickUp, 4095, true)],
            Some(&circular),
            &mut priorities,
        );
        assert!(close(up.sticks[0].x, 0.0) && close(up.sticks[0].y, -1.0));
        let diagonal = compute(
            Controller::Xbox,
            [
                (Control::LeftStickUp, 4095, true),
                (Control::LeftStickLeft, 4095, true),
            ],
            Some(&circular),
            &mut priorities,
        );
        let half = std::f32::consts::FRAC_1_SQRT_2;
        assert!(close(diagonal.sticks[0].x, -half) && close(diagonal.sticks[0].y, -half));
        let square = settings(Normalization::Square, false, None);
        let corner = compute(
            Controller::Xbox,
            [
                (Control::RightStickDown, 4095, true),
                (Control::RightStickRight, 4095, true),
            ],
            Some(&square),
            &mut priorities,
        );
        assert!(close(corner.sticks[1].x, 1.0) && close(corner.sticks[1].y, 1.0));
    }

    #[test]
    fn the_up_bias_weights_the_left_stick_pointing_up() {
        let biased = settings(Normalization::Square, false, Some(25));
        let shaped = shape(1.0, -1.0, true, 0.0, Some(&biased));
        assert!(close(shaped.x, 1.0 / 3.0) && close(shaped.y, -1.0));
        let right = shape(1.0, -1.0, false, 0.0, Some(&biased));
        assert!(close(right.x, 1.0) && close(right.y, -1.0));
    }

    #[test]
    fn triggers_follow_the_curve_and_stretch_the_top_end() {
        let raised = Settings {
            curve: [(51, 128), (102, 160), (153, 200), (204, 255)]
                .map(|(travel, output)| CurvePoint { travel, output }),
            ..settings(Normalization::Circular, false, None)
        };
        assert!(close(trigger_value(500, Some(&raised)), 0.0));
        assert!(close(
            (trigger_value(900, Some(&raised)) * 255.0).floor(),
            131.0
        ));
        assert!(close(trigger_value(4095, Some(&raised)), 1.0));
        assert!(close((trigger_value(2048, None) * 255.0).floor(), 127.0));
    }

    #[test]
    fn snappy_axes_follow_the_deeper_key_and_bottomed_out_keys_take_over() {
        let snappy = settings(Normalization::Circular, true, None);
        let mut priority = Priority::default();
        assert!(close(
            combine(3000, 1000, &mut priority, Some(&snappy)),
            3000.0 / 4095.0
        ));
        assert!(close(
            combine(4095, 4095, &mut priority, Some(&snappy)),
            -1.0
        ));
        assert!(close(
            combine(4095, 4095, &mut priority, Some(&snappy)),
            -1.0
        ));
        let blended = settings(Normalization::Circular, false, None);
        assert!(close(
            combine(3000, 1000, &mut priority, Some(&blended)),
            2000.0 / 4095.0
        ));
    }

    #[test]
    fn classic_sends_buttons_axes_and_one_hat_direction_per_axis() {
        let linear = settings(Normalization::Circular, false, None);
        let mut priorities = Priorities::default();
        let output = compute(
            Controller::Classic,
            [
                (Control::ButtonB, 4095, true),
                (Control::ButtonA, 4095, true),
                (Control::Guide, 4095, true),
                (Control::HatUp, 4095, true),
                (Control::HatDown, 4095, true),
                (Control::HatLeft, 4095, true),
                (Control::RightStickRight, 4095, true),
                (Control::Accelerator, 2048, false),
            ],
            Some(&linear),
            &mut priorities,
        );
        assert_eq!(
            output.pressed,
            vec![Control::ButtonA, Control::ButtonB, Control::HatLeft]
        );
        let value = |control| {
            output
                .axes
                .iter()
                .find(|axis| axis.control == control)
                .map(|axis| axis.value)
        };
        assert!(value(Control::RightStickRight).is_some_and(|value| close(value, 1.0)));
        assert!(value(Control::Accelerator).is_some_and(|value| close(value, 2048.0 / 4095.0)));
    }

    #[test]
    fn released_keys_rest_the_controller() {
        let mut priorities = Priorities::default();
        let output = compute(
            Controller::Xbox,
            [
                (Control::ButtonA, 0, false),
                (Control::LeftTrigger, 0, false),
            ],
            None,
            &mut priorities,
        );
        assert_eq!(output, rest(Controller::Xbox));
    }
}
