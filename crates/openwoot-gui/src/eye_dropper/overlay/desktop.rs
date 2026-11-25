#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
pub(super) use self::macos::{denied, profile};
#[cfg(target_os = "windows")]
pub(super) use self::windows::{denied, profile};
