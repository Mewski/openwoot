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
                .with_context(|| format!("could not read `{}`", file.display()))?;
            for name in names_in(&text, &names) {
                problems.push(format!(
                    "{}: shared code names the board or chip `{name}`",
                    relative(&workspace.root, file)
                ));
            }
        }
    }
    Ok(())
}

fn names_in<'n>(text: &str, names: &'n BTreeSet<String>) -> impl Iterator<Item = &'n str> {
    let text = text.to_lowercase();
    names
        .iter()
        .map(String::as_str)
        .filter(move |name| text.contains(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_board_or_chip_name_is_found_in_any_case() {
        let names = BTreeSet::from(["gd32f4", "wooting-80he", "wooting_80he"].map(str::to_owned));
        assert_eq!(
            names_in("use Wooting_80HE::scan;", &names).collect::<Vec<_>>(),
            ["wooting_80he"]
        );
        assert_eq!(
            names_in("// GD32F4 and wooting-80he", &names).collect::<Vec<_>>(),
            ["gd32f4", "wooting-80he"]
        );
        assert_eq!(names_in("let chip = 1;", &names).count(), 0);
    }
}
