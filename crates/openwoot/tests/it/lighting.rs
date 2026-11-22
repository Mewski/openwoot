#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    reason = "the helpers build fixed test tables and parse the golden data, and fail the test on a malformed line"
)]

mod effects;
mod engine;
mod host_control;
mod idle;
mod indicators;
mod lightbar;
mod support;
mod task;

pub(crate) use support::{
    COLUMNS, GEOMETRY, LENGTH, MAP, PAGES, ROWS, VARIANT, base_color, lighting,
};
