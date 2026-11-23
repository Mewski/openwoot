use std::{env, ffi::OsStr, path::Path, process::Command};

use anyhow::{Context, Result, ensure};

const NIGHTLY_VARIABLE: &str = "OPENWOOT_NIGHTLY";
const NIGHTLY: &str = "nightly";

pub(crate) fn cargo(root: &Path) -> Command {
    let mut command = Command::new("cargo");
    command.current_dir(root);
    command
}

pub(crate) fn nightly() -> String {
    let toolchain = env::var(NIGHTLY_VARIABLE).unwrap_or_else(|_| NIGHTLY.to_owned());
    format!("+{toolchain}")
}

pub(crate) fn execute(command: &mut Command) -> Result<()> {
    let shown = shown(command);
    let status = command
        .status()
        .with_context(|| format!("could not run `{shown}`"))?;
    ensure!(status.success(), "`{shown}` ended with {status}");
    Ok(())
}

fn shown(command: &Command) -> String {
    let words: Vec<_> = [command.get_program()]
        .into_iter()
        .chain(command.get_args())
        .map(OsStr::to_string_lossy)
        .collect();
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_shown_as_it_would_be_typed() {
        let mut command = Command::new("cargo");
        command.args(["clippy", "--package", "wooting-80he"]);
        assert_eq!(shown(&command), "cargo clippy --package wooting-80he");
    }
}
