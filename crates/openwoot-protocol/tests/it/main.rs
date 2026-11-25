#![allow(
    long_running_const_eval,
    reason = "postcard-rpc checks every schema of the endpoint table for unique keys at compile time"
)]

mod endpoint_keys;
mod messages;
mod reports;
mod round_trips;
mod support;
mod validation;
