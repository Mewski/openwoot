use std::path::Path;

use anyhow::Result;

use crate::{
    firmware,
    output::show,
    workspace::{Board, Workspace},
};

#[derive(clap::Args)]
pub(crate) struct Args {
    #[arg(help = "Board to build", required_unless_present = "all")]
    board: Option<String>,
    #[arg(long, help = "Build every board", conflicts_with = "board")]
    all: bool,
}

pub(crate) fn run(root: &Path, args: &Args) -> Result<()> {
    let workspace = Workspace::load(root)?;
    let boards: Vec<&Board> = if args.all {
        workspace.boards().iter().collect()
    } else {
        vec![workspace.board(args.board.as_deref().unwrap_or_default())?]
    };
    let width = boards
        .iter()
        .map(|board| board.name.len() + 1)
        .max()
        .unwrap_or_default();
    for board in boards {
        let images = firmware::build(&workspace, board)?;
        let image = images.hex.strip_prefix(root).unwrap_or(&images.hex);
        show(&board.name, image.display(), width);
    }
    Ok(())
}
