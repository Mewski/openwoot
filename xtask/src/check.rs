mod board_image;
mod names;
mod packages;
mod rust_rules;
mod rust_source;
mod symmetry;
mod tools;
mod tree;

use std::path::Path;

use anyhow::{Context, Result, bail};
use packages::Kind;

use crate::{output::show, workspace::Workspace};

pub(crate) fn run(root: &Path) -> Result<()> {
    let workspace = Workspace::load(root)?;
    let members = packages::classify(&workspace)?;
    let files = tree::files(root)?;

    let mut problems = Vec::new();
    tree::forbidden_files(root, &files, &mut problems);
    packages::dependency_direction(&members, &mut problems);
    packages::lints_tables(&members, &mut problems)?;
    symmetry::file_sets(&members, Kind::ChipHal, &files, &mut problems);
    symmetry::file_sets(&members, Kind::Board, &files, &mut problems);
    names::in_shared_code(&workspace, &members, &files, &mut problems)?;
    rust_rules::check(root, &members, &files, &mut problems)?;
    if !problems.is_empty() {
        for problem in &problems {
            eprintln!("{problem}");
        }
        bail!("{} problems in the tree", problems.len());
    }

    tools::formatting(root)?;
    tools::clippy(&workspace, &members)?;
    for board in workspace.boards() {
        board_image::check(&workspace, board)
            .with_context(|| format!("could not check the image of `{}`", board.name))?;
    }
    show("check", "passed", 0);
    Ok(())
}

fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .display()
        .to_string()
}
