mod board_image;
mod budget;
mod call_graph;
mod entry_points;
mod names;
mod packages;
mod panic_paths;
mod rust_rules;
mod rust_source;
mod symmetry;
mod tools;
mod tree;

use std::{collections::BTreeSet, path::Path};

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
    stop_on(&problems, "tree")?;

    tools::formatting(root)?;
    tools::clippy(&workspace, &members)?;
    let width = workspace
        .boards()
        .iter()
        .map(|board| board.name.len() + 1)
        .max()
        .unwrap_or_default();
    let mut reviewed = BTreeSet::new();
    for board in workspace.boards() {
        let analysis = board_image::check(&workspace, board)
            .with_context(|| format!("could not check the image of `{}`", board.name))?;
        show(&board.name, analysis.budget, width);
        if let Err(error) = analysis.budget.check() {
            problems.push(format!("`{}`: {error}", board.name));
        }
        for problem in analysis.panics.problems {
            problems.push(format!("`{}`: panic reachable: {problem}", board.name));
        }
        reviewed.extend(analysis.panics.reviewed);
    }
    for (index, site) in panic_paths::REVIEWED.iter().enumerate() {
        if !reviewed.contains(&index) {
            problems.push(format!(
                "reviewed panic site {site} occurs in no image; remove it"
            ));
        }
    }
    stop_on(&problems, "images")?;
    show("check", "passed", 0);
    Ok(())
}

fn stop_on(problems: &[String], place: &str) -> Result<()> {
    if problems.is_empty() {
        return Ok(());
    }
    for problem in problems {
        eprintln!("{problem}");
    }
    let noun = if problems.len() == 1 {
        "problem"
    } else {
        "problems"
    };
    bail!("{} {noun} in the {place}", problems.len())
}

fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .display()
        .to_string()
}
