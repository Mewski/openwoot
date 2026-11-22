mod boards;
mod build;
mod check;
mod elf;
mod firmware;
mod flash;
mod output;
mod pac;
mod process;
mod workspace;

use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Build helper of the openwoot workspace")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "List the boards (packages under `boards/`)")]
    Boards(boards::Args),
    #[command(about = "Build firmware images into `build/<board>/` (ELF, Intel HEX, binary)")]
    Build(build::Args),
    #[command(about = "Build a board and hand its image to `openwoot flash`")]
    Flash(flash::Args),
    #[command(
        about = "Run every check of the tree: files, dependencies, code rules, formatting, lints, images"
    )]
    Check,
    #[command(about = "Regenerate the peripheral access crates from their SVD files")]
    Pac,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let root = workspace_root()?;
    match cli.command {
        Command::Boards(args) => boards::run(&root, &args),
        Command::Build(args) => build::run(&root, &args),
        Command::Flash(args) => flash::run(&root, &args),
        Command::Check => check::run(&root),
        Command::Pac => pac::run(&root),
    }
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask lives outside the workspace")
}
