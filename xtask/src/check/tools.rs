use std::{path::Path, process::Command};

use anyhow::Result;

use super::packages::{Kind, Member};
use crate::{process, workspace::Workspace};

pub(super) fn formatting(root: &Path) -> Result<()> {
    process::execute(
        process::cargo(root)
            .env_remove("RUSTUP_TOOLCHAIN")
            .arg(process::nightly())
            .args(["fmt", "--all", "--", "--check"]),
    )?;
    process::execute(
        Command::new("taplo")
            .current_dir(root)
            .args(["fmt", "--check"]),
    )?;
    process::execute(process::cargo(root).args(["sort", "--workspace", "--check"]))
}

pub(super) fn clippy(workspace: &Workspace, members: &[Member<'_>]) -> Result<()> {
    let mut host = process::cargo(&workspace.root);
    host.args(["clippy", "--workspace", "--all-targets"]);
    for member in members
        .iter()
        .filter(|member| matches!(member.kind, Kind::Pac | Kind::Board | Kind::BoardFirmware))
    {
        host.args(["--exclude", member.package.name.as_str()]);
    }
    process::execute(host.args(["--", "-D", "warnings"]))?;

    let mut board_libraries = process::cargo(&workspace.root);
    board_libraries.args(["clippy", "--lib", "--tests"]);
    for board in workspace.boards() {
        board_libraries.args(["--package", &board.name]);
    }
    process::execute(board_libraries.args(["--", "-D", "warnings"]))?;

    for board in workspace.boards() {
        process::execute(process::cargo(&workspace.root).args([
            "clippy",
            "--package",
            &board.name,
            "--package",
            &board.firmware,
            "--package",
            &board.chip_hal,
            "--target",
            &board.target,
            "--",
            "-D",
            "warnings",
        ]))?;
    }
    Ok(())
}
