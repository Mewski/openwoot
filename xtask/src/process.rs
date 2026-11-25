use std::{
    env,
    ffi::OsStr,
    io::BufReader,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, ensure};
use cargo_metadata::Message;

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

pub(crate) fn messages(command: &mut Command) -> Result<Vec<Message>> {
    command
        .arg("--message-format=json-render-diagnostics")
        .stdout(Stdio::piped());
    let shown = shown(command);
    let mut child = command
        .spawn()
        .with_context(|| format!("could not run `{shown}`"))?;
    let output = child
        .stdout
        .take()
        .with_context(|| format!("`{shown}` has no output"))?;
    let messages = Message::parse_stream(BufReader::new(output))
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("could not read the messages of `{shown}`"))?;
    let status = child
        .wait()
        .with_context(|| format!("could not run `{shown}`"))?;
    ensure!(status.success(), "`{shown}` ended with {status}");
    Ok(messages)
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
