use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use super::relative;

const FORBIDDEN_EXTENSIONS: [&str; 19] = [
    "asm", "bash", "bat", "c", "cc", "cmd", "cpp", "cxx", "fish", "h", "hh", "hpp", "hxx", "mk",
    "ps1", "py", "s", "sh", "zsh",
];
const FORBIDDEN_FILE_NAMES: [&str; 4] = ["CMakeLists.txt", "GNUmakefile", "Makefile", "makefile"];

pub(super) fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let unreadable = || format!("could not read `{}`", directory.display());
        for entry in fs::read_dir(&directory).with_context(unreadable)? {
            let path = entry.with_context(unreadable)?.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let skipped = name.starts_with('.') && name != ".cargo" && name != ".github"
                || name == "target"
                || path == root.join("build");
            if skipped {
                continue;
            }
            if path.is_dir() {
                directories.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn forbidden_files(root: &Path, files: &[PathBuf], problems: &mut Vec<String>) {
    for file in files {
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let extension = file
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        if FORBIDDEN_FILE_NAMES.contains(&name)
            || FORBIDDEN_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        {
            problems.push(format!(
                "{}: not a Rust tree file type",
                relative(root, file)
            ));
        }
    }
}
