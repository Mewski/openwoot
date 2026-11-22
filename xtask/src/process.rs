use std::{path::Path, process::Command};

use anyhow::{Context, Result, ensure};

pub(crate) fn cargo(root: &Path) -> Command {
    let mut command = Command::new("cargo");
    command.current_dir(root);
    command
}

pub(crate) fn execute(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("could not run {command:?}"))?;
    ensure!(status.success(), "{command:?} ended with {status}");
    Ok(())
}
