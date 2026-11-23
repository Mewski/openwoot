use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use super::{
    packages::{Kind, Member},
    relative,
    rust_source::{self, Comment, Scanned},
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
        let unsafe_lines = scanned.keyword_lines(UNSAFE_KEYWORD);
        for comment in &scanned.comments {
            let fault = if is_in_safety_note(&scanned, comment.line) {
                note_fault(&scanned, &unsafe_lines, comment)
            } else {
                Some("comment outside a `// SAFETY:` note")
            };
            if let Some(fault) = fault {
                problems.push(format!("{shown}:{}: {fault}", comment.line));
            }
        }
        for line in unsafe_lines {
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
    let module = file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
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

fn note_fault(
    scanned: &Scanned,
    unsafe_lines: &[usize],
    comment: &Comment,
) -> Option<&'static str> {
    let reason = comment.text.strip_prefix(SAFETY_NOTE)?;
    if reason.trim().is_empty() {
        return Some("`// SAFETY:` note without a reason on its first line");
    }
    let covers = |line: usize| unsafe_lines.contains(&line);
    if !is_comment_only(scanned, comment.line) {
        return (!covers(comment.line)).then_some("`// SAFETY:` note on a line without `unsafe`");
    }
    let mut next = comment.line + 1;
    while is_comment_only(scanned, next) {
        next += 1;
    }
    while !covers(next) {
        if !scanned.code_on(next).trim_start().starts_with("#[") {
            return Some("`// SAFETY:` note not followed by `unsafe`");
        }
        next += 1;
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    fn faults(source: &str) -> Vec<(usize, &'static str)> {
        let scanned = rust_source::scan(source);
        let unsafe_lines = scanned.keyword_lines(UNSAFE_KEYWORD);
        scanned
            .comments
            .iter()
            .filter(|comment| is_in_safety_note(&scanned, comment.line))
            .filter_map(|comment| {
                note_fault(&scanned, &unsafe_lines, comment).map(|fault| (comment.line, fault))
            })
            .collect()
    }

    #[test]
    fn a_note_directly_above_unsafe_code_is_accepted() {
        let source = "// SAFETY: the register block is valid\n// for the whole program.\n\
                      let block = unsafe { &*BLOCK };\n\
                      // SAFETY: the symbol is unique\n#[unsafe(no_mangle)]\nfn handler() {}\n\
                      // SAFETY: interrupts are bound once\n#[inline]\nunsafe fn bind() {}\n\
                      let value = unsafe { read() }; // SAFETY: the pointer is aligned\n";
        assert_eq!(faults(source), [] as [(usize, &str); 0]);
        assert!(has_safety_note(&rust_source::scan(source), 3));
    }

    #[test]
    fn a_note_without_unsafe_below_it_is_reported() {
        let source = "// SAFETY: nothing here\n// TODO: later\nlet value = 1;\n\n\
                      // SAFETY: separated by a blank line\n\nunsafe { run() }\n";
        assert_eq!(
            faults(source),
            [
                (1, "`// SAFETY:` note not followed by `unsafe`"),
                (5, "`// SAFETY:` note not followed by `unsafe`"),
            ]
        );
    }

    #[test]
    fn an_empty_note_and_a_note_beside_safe_code_are_reported() {
        let source = "// SAFETY:\nunsafe { run() }\nlet value = 1; // SAFETY: no unsafe here\n";
        assert_eq!(
            faults(source),
            [
                (1, "`// SAFETY:` note without a reason on its first line"),
                (3, "`// SAFETY:` note on a line without `unsafe`"),
            ]
        );
    }

    #[test]
    fn comments_outside_a_note_are_not_notes() {
        let scanned = rust_source::scan("// a plain comment\nlet value = 1;\n");
        assert!(!is_in_safety_note(&scanned, 1));
    }
}
