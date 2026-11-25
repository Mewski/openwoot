use std::{env, io};

use auto_launch::{AutoLaunch, AutoLaunchBuilder, MacOSLaunchMode, WindowsEnableMode};

pub(crate) const HIDDEN_FLAG: &str = "--background";
const NAME: &str = "openwoot";

#[derive(Debug, thiserror::Error)]
pub(crate) enum LaunchError {
    #[error("could not find the openwoot-gui program")]
    Program(#[source] io::Error),
    #[error("could not change whether openwoot starts with the computer")]
    Register(#[source] auto_launch::Error),
}

pub(crate) fn set_start_with_computer(on: bool) -> Result<(), LaunchError> {
    let entry = entry()?;
    let registered = entry.is_enabled().unwrap_or(false);
    let changed = match (on, registered) {
        (true, _) => entry.enable(),
        (false, true) => entry.disable(),
        (false, false) => Ok(()),
    };
    changed.map_err(LaunchError::Register)
}

pub(crate) fn starts_with_computer() -> bool {
    entry()
        .ok()
        .and_then(|entry| entry.is_enabled().ok())
        .unwrap_or(false)
}

fn entry() -> Result<AutoLaunch, LaunchError> {
    let program = env::current_exe().map_err(LaunchError::Program)?;
    AutoLaunchBuilder::new()
        .set_app_name(NAME)
        .set_app_path(&program.to_string_lossy())
        .set_macos_launch_mode(MacOSLaunchMode::LaunchAgent)
        .set_windows_enable_mode(WindowsEnableMode::CurrentUser)
        .set_args(&[HIDDEN_FLAG])
        .build()
        .map_err(LaunchError::Register)
}
