#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "the gamepad math converts as the C firmware does: travel to float, floats to integers \
              truncated toward zero and saturated, then cut to the width of the report field"
)]

mod axis;
mod curve;
mod gamepad;
mod input;
mod key_code;
mod stick;
mod trigger;

pub use axis::{AxisPriority, combine_axis};
pub use curve::{normalize_travel, response_curve};
pub use gamepad::Gamepad;
pub use input::GamepadInput;
pub use key_code::follows_travel;
pub(crate) use key_code::gamepad_code;
pub use stick::shape_stick;
