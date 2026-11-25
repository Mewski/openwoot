#[cfg(any(target_os = "macos", target_os = "windows"))]
mod overlay;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod portal;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) use self::overlay::{Error, pick};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(crate) use self::portal::{Error, pick};
