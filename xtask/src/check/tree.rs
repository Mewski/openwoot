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
const SKIPPED_AT_ROOT: [&str; 2] = ["build", "target"];
const CHECKED_HIDDEN_AT_ROOT: [&str; 2] = [".cargo", ".github"];

pub(super) fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let unreadable = || format!("could not read `{}`", directory.display());
        let at_root = directory.as_path() == root;
        for entry in fs::read_dir(&directory).with_context(unreadable)? {
            let path = entry.with_context(unreadable)?.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if at_root && is_skipped_at_root(name) {
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

fn is_skipped_at_root(name: &str) -> bool {
    SKIPPED_AT_ROOT.contains(&name)
        || (name.starts_with('.') && !CHECKED_HIDDEN_AT_ROOT.contains(&name))
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

#[cfg(test)]
mod tests {
    use std::{env, process};

    use super::*;

    #[test]
    fn the_walk_skips_build_output_and_hidden_directories_at_the_root_only() {
        let root = env::temp_dir().join(format!("openwoot-xtask-tree-{}", process::id()));
        let _ = fs::remove_dir_all(&root);
        for directory in [
            "target/debug",
            "build/board",
            ".git",
            ".github/workflows",
            "crates/core/src/target",
            "crates/core/src/.hidden",
        ] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        for file in [
            "target/debug/generated.py",
            "build/board/image.sh",
            ".git/hook.py",
            ".gitignore",
            ".github/workflows/ci.yml",
            "crates/core/src/target/probe.py",
            "crates/core/src/.hidden/module.rs",
            "Cargo.toml",
        ] {
            fs::write(root.join(file), "").unwrap();
        }
        let found: Vec<String> = files(&root)
            .unwrap()
            .iter()
            .map(|file| relative(&root, file))
            .collect();
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            found,
            [
                ".github/workflows/ci.yml",
                "Cargo.toml",
                "crates/core/src/.hidden/module.rs",
                "crates/core/src/target/probe.py",
            ]
        );
    }

    #[test]
    fn scripts_and_make_files_are_reported() {
        let root = Path::new("/tree");
        let files: Vec<PathBuf> = [
            "tools/gen.py",
            "Makefile",
            "scripts/Run.SH",
            "src/lib.rs",
            "Cargo.toml",
            "native/CMakeLists.txt",
        ]
        .iter()
        .map(|file| root.join(file))
        .collect();
        let mut problems = Vec::new();
        forbidden_files(root, &files, &mut problems);
        assert_eq!(
            problems,
            [
                "tools/gen.py: not a Rust tree file type",
                "Makefile: not a Rust tree file type",
                "scripts/Run.SH: not a Rust tree file type",
                "native/CMakeLists.txt: not a Rust tree file type",
            ]
        );
    }
}
