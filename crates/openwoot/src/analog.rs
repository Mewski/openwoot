#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "the analog model reproduces the fixed-width integer and float arithmetic of the C firmware"
)]

mod calibration;
mod curve;
mod noise;
mod rebound;
mod recalibration;
mod sensor;
mod switch;

pub use calibration::BottomOut;
pub use curve::Curve;
pub use noise::{Calibration, MinMax, NoiseState, NoiseStatistics, StdDev};
pub use rebound::{Rebound, ReboundState};
pub(crate) use recalibration::{Advance, Decision};
pub use recalibration::{Batching, Recalibration};
pub use sensor::Sensor;
pub use switch::{Switch, SwitchType};
