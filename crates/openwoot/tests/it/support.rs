mod board;
mod configuration;
mod enumeration;
mod report_descriptor;
mod wire;

pub(crate) use board::{EXAMPLE_SERIAL, Recorder, stock_identity};
pub(crate) use configuration::parse as parse_configuration;
pub(crate) use enumeration::{IDENTITY, configuration, enumerate, faces};
pub(crate) use report_descriptor::{Kind, Usages, parse as parse_report_descriptor};
pub(crate) use wire::{Host, Outcome, poll, setup};
