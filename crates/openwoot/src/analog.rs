#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "the analog model converts between fixed-width integers and floats on purpose"
)]

mod bottom_out;
mod curve;
mod noise;
mod rebound;
mod switch;
mod switch_model;

pub(crate) use bottom_out::BottomOut;
use bottom_out::VALUE_NONE;
use curve::Curve;
pub use noise::{Calibration, MinMax, NoiseState, NoiseStatistics, StandardDeviation};
pub use rebound::{Rebound, ReboundState};
pub use switch::Switch;
pub use switch_model::SwitchModel;
