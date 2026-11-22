mod check;
mod firmware;
mod pac;
mod rust_source;
mod workspace;

use std::{
    fmt::Display,
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use crate::workspace::Workspace;

#[derive(Parser)]
#[command(about = "Build helper of the openwoot workspace")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "List the boards (packages under `boards/`)")]
    Boards {
        #[arg(long, help = "Print a JSON array, for a CI matrix")]
        json: bool,
    },
    #[command(about = "Build firmware images into `build/<board>/` (ELF, Intel HEX, binary)")]
    Build {
        #[arg(help = "Board to build", required_unless_present = "all")]
        board: Option<String>,
        #[arg(long, help = "Build every board", conflicts_with = "board")]
        all: bool,
    },
    #[command(about = "Build a board and hand its image to `openwoot flash`")]
    Flash {
        #[arg(help = "Board to flash")]
        board: String,
        #[arg(
            long,
            help = "Check the image and print what would be sent, without touching a device"
        )]
        dry_run: bool,
    },
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
        Command::Boards { json } => {
            let workspace = Workspace::load(&root)?;
            let names: Vec<&str> = workspace
                .boards()
                .iter()
                .map(|board| board.name.as_str())
                .collect();
            if json {
                let quoted: Vec<String> = names.iter().map(|name| format!("\"{name}\"")).collect();
                println!("[{}]", quoted.join(","));
            } else {
                for name in &names {
                    println!("{name}");
                }
            }
            Ok(())
        }
        Command::Build { board, all } => {
            let workspace = Workspace::load(&root)?;
            let boards = if all {
                workspace.boards().to_vec()
            } else {
                vec![
                    workspace
                        .board(board.as_deref().unwrap_or_default())?
                        .clone(),
                ]
            };
            let width = boards
                .iter()
                .map(|board| board.name.len() + 1)
                .max()
                .unwrap_or_default();
            for board in &boards {
                let images = firmware::build(&workspace, board)?;
                let image = images.hex.strip_prefix(&root).unwrap_or(&images.hex);
                show(&board.name, image.display(), width);
            }
            Ok(())
        }
        Command::Flash { board, dry_run } => {
            let workspace = Workspace::load(&root)?;
            let images = firmware::build(&workspace, workspace.board(&board)?)?;
            let mut command = workspace::cargo(&root);
            command
                .args(["run", "--quiet", "--package", "openwoot-cli", "--", "flash"])
                .arg(&images.hex);
            if dry_run {
                command.arg("--dry-run");
            }
            workspace::execute(&mut command)
        }
        Command::Check => check::run(&root),
        Command::Pac => pac::run(&root),
    }
}

pub(crate) fn show(label: &str, value: impl Display, width: usize) {
    println!("{:<width$} {value}", format!("{label}:"));
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .context("xtask lives outside the workspace")
}
