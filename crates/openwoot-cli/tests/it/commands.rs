use predicates::{prelude::PredicateBooleanExt, str::contains};

use crate::support::openwoot;

#[test]
fn help_lists_exactly_version_and_flash() {
    let assert = openwoot().arg("--help").assert().success();
    let output = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    let commands: Vec<&str> = output
        .lines()
        .skip_while(|line| !line.starts_with("Commands:"))
        .skip(1)
        .take_while(|line| !line.is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    assert_eq!(commands, ["version", "flash", "help"]);
}

#[test]
fn a_command_is_required() {
    openwoot()
        .assert()
        .failure()
        .stderr(contains("Usage: openwoot <COMMAND>"));
}

#[test]
fn unknown_commands_are_rejected() {
    for command in ["info", "enter", "reboot", "plan"] {
        openwoot()
            .arg(command)
            .assert()
            .failure()
            .stderr(contains("unrecognized subcommand"));
    }
}

#[test]
fn flash_needs_an_image() {
    openwoot()
        .arg("flash")
        .assert()
        .failure()
        .stderr(contains("<IMAGE>"));
}

#[test]
fn flash_takes_only_dry_run() {
    openwoot()
        .args(["flash", "image.hex", "--yes"])
        .assert()
        .failure()
        .stderr(contains("unexpected argument '--yes'"));
}

#[test]
fn version_takes_no_arguments() {
    openwoot()
        .args(["version", "extra"])
        .assert()
        .failure()
        .stderr(contains("unexpected argument 'extra'"));
}

#[test]
fn flash_help_describes_dry_run() {
    openwoot()
        .args(["flash", "--help"])
        .assert()
        .success()
        .stdout(contains("--dry-run").and(contains("<IMAGE>")));
}
