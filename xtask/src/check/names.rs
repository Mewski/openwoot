use std::{collections::BTreeSet, fs, path::PathBuf};

use anyhow::{Context, Result};

use super::{
    packages::{Kind, Member},
    relative,
};
use crate::workspace::Workspace;

pub(super) fn in_shared_code(
    workspace: &Workspace,
    members: &[Member<'_>],
    files: &[PathBuf],
    problems: &mut Vec<String>,
) -> Result<()> {
    let mut names = BTreeSet::new();
    for member in members {
        let name = member.package.name.as_str();
        match member.kind {
            Kind::Board => {
                names.insert(name.to_owned());
                names.insert(name.replace('-', "_"));
            }
            Kind::ChipHal => {
                names.insert(name.trim_start_matches("openwoot-").to_owned());
            }
            Kind::Pac => {
                names.insert(name.trim_end_matches("-pac").to_owned());
            }
            _ => {}
        }
    }
    for member in members.iter().filter(|member| member.kind.is_shared()) {
        for file in files
            .iter()
            .filter(|file| file.starts_with(&member.directory))
        {
            let text = fs::read_to_string(file)
                .with_context(|| format!("could not read `{}`", file.display()))?
                .to_lowercase();
            for name in names.iter().filter(|name| text.contains(name.as_str())) {
                problems.push(format!(
                    "{}: shared code names the board or chip `{name}`",
                    relative(&workspace.root, file)
                ));
            }
        }
    }
    Ok(())
}
