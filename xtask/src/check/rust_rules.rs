use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use super::{
    packages::{Kind, Member},
    relative,
    rust_source::{self, Scanned},
};

const VAGUE_MODULE_NAMES: [&str; 5] = ["common", "helpers", "misc", "types", "utils"];
const SAFETY_NOTE: &str = "// SAFETY:";
const UNSAFE_KEYWORD: &str = "unsafe";

pub(super) fn check(
    root: &Path,
    members: &[Member<'_>],
    files: &[PathBuf],
    problems: &mut Vec<String>,
) -> Result<()> {
    for file in files
        .iter()
        .filter(|file| file.extension().is_some_and(|extension| extension == "rs"))
    {
        let owner = members
            .iter()
            .filter(|member| file.starts_with(&member.directory))
            .max_by_key(|member| member.directory.components().count());
        let kind = owner.map(|member| member.kind);
        if kind == Some(Kind::Pac) {
            continue;
        }
        let shown = relative(root, file);
        if is_vague_module(file) {
            problems.push(format!(
                "{shown}: module name says nothing about its content"
            ));
        }
        let source = fs::read_to_string(file)
            .with_context(|| format!("could not read `{}`", file.display()))?;
        let scanned = rust_source::scan(&source);
        for comment in &scanned.comments {
            if !is_in_safety_note(&scanned, comment.line) {
                problems.push(format!(
                    "{shown}:{}: comment outside a `// SAFETY:` note",
                    comment.line
                ));
            }
        }
        for line in scanned.keyword_lines(UNSAFE_KEYWORD) {
            if kind != Some(Kind::ChipHal) {
                problems.push(format!("{shown}:{line}: `unsafe` outside a chip HAL"));
            } else if !has_safety_note(&scanned, line) {
                problems.push(format!(
                    "{shown}:{line}: `unsafe` without a `// SAFETY:` note"
                ));
            }
        }
    }
    Ok(())
}

fn is_vague_module(file: &Path) -> bool {
    let stem = file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let module = if stem == "mod" {
        file.parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    } else {
        stem
    };
    VAGUE_MODULE_NAMES.contains(&module)
}

fn is_comment_only(scanned: &Scanned, line: usize) -> bool {
    scanned.code_on(line).trim().is_empty()
        && scanned
            .comment_on(line)
            .is_some_and(|comment| comment.text.starts_with("//"))
}

fn is_in_safety_note(scanned: &Scanned, line: usize) -> bool {
    let mut start = line;
    while start > 1 && is_comment_only(scanned, start) && is_comment_only(scanned, start - 1) {
        start -= 1;
    }
    scanned
        .comment_on(start)
        .is_some_and(|comment| comment.text.starts_with(SAFETY_NOTE))
}

fn has_safety_note(scanned: &Scanned, line: usize) -> bool {
    if scanned
        .comment_on(line)
        .is_some_and(|comment| comment.text.starts_with(SAFETY_NOTE))
    {
        return true;
    }
    let mut above = line.saturating_sub(1);
    while above > 0 && scanned.code_on(above).trim_start().starts_with("#[") {
        above -= 1;
    }
    above > 0 && is_comment_only(scanned, above) && is_in_safety_note(scanned, above)
}
