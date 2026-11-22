use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use super::packages::{Kind, Member};

pub(super) fn file_sets(
    members: &[Member<'_>],
    kind: Kind,
    files: &[PathBuf],
    problems: &mut Vec<String>,
) {
    let sets: Vec<(&str, BTreeSet<&Path>)> = members
        .iter()
        .filter(|member| member.kind == kind)
        .map(|member| {
            let set = files
                .iter()
                .filter_map(|file| file.strip_prefix(&member.directory).ok())
                .filter(|file| is_symmetric(kind, file))
                .collect();
            (member.package.name.as_str(), set)
        })
        .collect();
    let Some(((first_name, first_set), others)) = sets.split_first() else {
        return;
    };
    for (name, set) in others {
        for file in first_set.symmetric_difference(set) {
            problems.push(format!(
                "{kind:?} crates differ: `{}` is in only one of `{first_name}` and `{name}`",
                file.display()
            ));
        }
    }
}

fn is_symmetric(kind: Kind, file: &Path) -> bool {
    let in_module_directory = file.starts_with("src") && file.components().count() > 2;
    !(kind == Kind::ChipHal && in_module_directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chip_hals_compare_top_level_files_and_boards_compare_every_file() {
        assert!(is_symmetric(Kind::ChipHal, Path::new("src/usb.rs")));
        assert!(is_symmetric(Kind::ChipHal, Path::new("Cargo.toml")));
        assert!(!is_symmetric(
            Kind::ChipHal,
            Path::new("src/usb/endpoint.rs")
        ));
        assert!(is_symmetric(Kind::Board, Path::new("src/usb/endpoint.rs")));
    }
}
