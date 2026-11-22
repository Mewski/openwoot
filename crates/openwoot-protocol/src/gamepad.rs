use heapless::Vec;
use postcard_schema::Schema;
use serde::{Deserialize, Serialize};

use crate::configuration::{Capabilities, Invalid, MAX_KEYS, Validate};

pub const CURVE_POINTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Control {
    ButtonA,
    ButtonB,
    ButtonX,
    ButtonY,
    Start,
    Back,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    LeftBumper,
    RightBumper,
    Guide,
    LeftStickUp,
    LeftStickDown,
    LeftStickLeft,
    LeftStickRight,
    RightStickUp,
    RightStickDown,
    RightStickLeft,
    RightStickRight,
    LeftStickPress,
    RightStickPress,
    LeftTrigger,
    RightTrigger,
    Accelerator,
    Rudder,
    Trigger,
    RxNegative,
    RxPositive,
    RyNegative,
    RyPositive,
    HatUp,
    HatDown,
    HatLeft,
    HatRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Mapping {
    pub bindings: Vec<Option<Control>, MAX_KEYS>,
    pub exclusive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct Settings {
    pub curve: [CurvePoint; CURVE_POINTS],
    pub normalization: Normalization,
    pub single_direction_axes: bool,
    pub left_stick_up_bias: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub struct CurvePoint {
    pub travel: u8,
    pub output: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Schema)]
pub enum Normalization {
    #[default]
    Circular,
    Square,
}

impl Validate for Mapping {
    fn validate(&self, capabilities: &Capabilities) -> Result<(), Invalid> {
        capabilities.check_key_count(self.bindings.len())
    }
}

impl Validate for Settings {
    fn validate(&self, _: &Capabilities) -> Result<(), Invalid> {
        let rising = self
            .curve
            .windows(2)
            .all(|pair| matches!(pair, [low, high] if low.travel <= high.travel));
        let spans = matches!(
            (self.curve.first(), self.curve.last()),
            (Some(dead_zone), Some(saturation)) if dead_zone.travel < saturation.travel
        );
        if !(rising && spans) {
            return Err(Invalid::CurveNotRising);
        }
        match self.left_stick_up_bias {
            Some(percent) if percent > 100 => Err(Invalid::Percentage(percent)),
            _ => Ok(()),
        }
    }
}
