use std::path::Path;

use anyhow::Result;

use crate::workspace::Workspace;

#[derive(clap::Args)]
pub(crate) struct Args {
    #[arg(long, help = "Print a JSON array, for a CI matrix")]
    json: bool,
}

pub(crate) fn run(root: &Path, args: &Args) -> Result<()> {
    let workspace = Workspace::load(root)?;
    let names = workspace.boards().iter().map(|board| board.name.as_str());
    if args.json {
        let quoted: Vec<String> = names.map(|name| format!("\"{name}\"")).collect();
        println!("[{}]", quoted.join(","));
    } else {
        for name in names {
            println!("{name}");
        }
    }
    Ok(())
}
