#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::inline_always,
    clippy::struct_excessive_bools,
    reason = "the scan interrupt inlines its per-sample steps and keeps a narrow scan state"
)]

mod converter;
mod events;
mod map;
mod mode;
mod position;
mod recalibration;
mod recheck;
mod reference;
mod scanner;
mod sensing;
mod sensor;
mod sensor_readout;
mod sensors;
mod shared;
mod travel;

pub use converter::{Converter, Sample};
pub use events::{Event, Events};
pub use map::Map;
pub use mode::Mode;
use position::Grid;
pub use position::{ADDRESSES_PER_INPUT, Position};
use recalibration::{Advance, Decision};
pub use recalibration::{Batching, Recalibration};
pub use recheck::{Recheck, RecheckFrom};
pub use reference::{MeasuredReference, Reference, ZeroLevel};
pub use scanner::{Outcome, Scanner};
pub use sensing::Sensing;
pub use sensor::Sensor;
use sensor::{Context, Update};
pub use sensor_readout::SensorReadout;
pub use sensors::Sensors;
pub use shared::Shared;
pub use travel::Travel;
