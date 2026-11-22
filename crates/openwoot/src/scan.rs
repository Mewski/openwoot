#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::inline_always,
    clippy::struct_excessive_bools,
    reason = "the scan interrupt inlines its per-sample steps and keeps the C firmware's narrow scan state"
)]

mod converter;
mod events;
mod map;
mod recheck;
mod reference;
mod scanner;
mod sensing;
mod shared;
mod travel;

pub use converter::{Converter, Sample};
pub use events::{EVENT_CAPACITY, Event, Events};
pub use map::{ADDRESSES_PER_INPUT, Map, MatrixKey, Position};
pub use recheck::{Recheck, RecheckFrom};
pub use reference::{MeasuredReference, Reference, ZeroLevel};
pub use scanner::{Mode, Outcome, Scanner};
pub use sensing::Sensing;
pub use shared::Shared;
pub use travel::Travel;
