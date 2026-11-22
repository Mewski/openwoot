#![cfg_attr(not(test), no_std)]
#![allow(unsafe_code, reason = "the chip HAL is the hardware boundary")]

mod basepri;
mod descriptor;
#[cfg(target_os = "none")]
pub mod executor;
pub mod gpio;
pub mod interrupt;
pub mod rcu;
mod register;
pub mod system;
pub mod time;

pub use gd32f303_pac as pac;
#[doc(hidden)]
pub use openwoot_image;
