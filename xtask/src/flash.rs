use std::path::Path;

use anyhow::Result;

use crate::{firmware, process, workspace::Workspace};

#[derive(clap::Args)]
pub(crate) struct Args {
    #[arg(help = "Board to flash")]
    board: String,
    #[arg(
        long,
        help = "Check the image and print what would be sent, without touching a device"
    )]
    dry_run: bool,
}

pub(crate) fn run(root: &Path, args: &Args) -> Result<()> {
    let workspace = Workspace::load(root)?;
    let images = firmware::build(&workspace, workspace.board(&args.board)?)?;
    let mut command = process::cargo(root);
    command
        .args(["run", "--quiet", "--package", "openwoot-cli", "--", "flash"])
        .arg(&images.hex);
    if args.dry_run {
        command.arg("--dry-run");
    }
    process::execute(&mut command)
}
