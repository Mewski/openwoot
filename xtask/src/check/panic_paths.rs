mod location;
mod reviewed;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use location::Location;
use openwoot_image::Hex;
pub(super) use reviewed::REVIEWED;

use crate::{
    call_graph::{CallGraph, Reference},
    elf::Elf,
    entry_points::EntryPoints,
};

const LOCATION_WINDOW: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Origin {
    Interrupt,
    KeyTask,
}

pub(super) struct Findings {
    pub(super) problems: Vec<String>,
    pub(super) reviewed: BTreeSet<usize>,
}

struct Site {
    entry: usize,
    roots: Vec<(usize, Origin)>,
}

pub(super) fn find(elf: &Elf<'_>, graph: &CallGraph<'_>, entry_points: &EntryPoints) -> Findings {
    let roots = entry_points
        .handlers
        .iter()
        .map(|&root| (root, Origin::Interrupt))
        .chain(
            entry_points
                .key_tasks
                .iter()
                .map(|&root| (root, Origin::KeyTask)),
        );
    let mut sites = BTreeMap::new();
    for (root, origin) in roots {
        for (caller, at, entry) in panic_calls(graph, root) {
            sites
                .entry((caller, at))
                .or_insert_with(|| Site {
                    entry,
                    roots: Vec::new(),
                })
                .roots
                .push((root, origin));
        }
    }
    let mut findings = Findings {
        problems: Vec::new(),
        reviewed: BTreeSet::new(),
    };
    for ((caller, at), Site { entry, roots }) in sites {
        let location = location_of(elf, graph, caller, at);
        let reviewed = location.as_ref().and_then(|location| {
            REVIEWED
                .iter()
                .position(|reviewed| reviewed.matches(location))
        });
        let covering = reviewed.and_then(|index| REVIEWED.get(index));
        let uncovered = roots
            .iter()
            .find(|(_, origin)| covering.is_none_or(|reviewed| reviewed.reached_from != *origin));
        let Some(&(root, _)) = uncovered else {
            findings.reviewed.extend(reviewed);
            continue;
        };
        let place = location.map_or_else(|| Hex(at).to_string(), |location| location.to_string());
        findings.problems.push(format!(
            "`{}` reaches `{}` in `{}` at `{place}`",
            graph.name(root),
            graph.name(entry),
            graph.name(caller)
        ));
    }
    findings
}

fn panic_calls(graph: &CallGraph<'_>, root: usize) -> Vec<(usize, u32, usize)> {
    let mut visited = vec![false; graph.len()];
    let mut queue = VecDeque::from([root]);
    let mut calls = Vec::new();
    if let Some(seen) = visited.get_mut(root) {
        *seen = true;
    }
    while let Some(node) = queue.pop_front() {
        for reference in graph.references(node) {
            let name = graph.name(reference.node);
            if is_panic_entry(name) {
                if !is_resumption_after_completion(name) {
                    calls.push((node, reference.at, reference.node));
                }
                continue;
            }
            if let Some(seen) = visited.get_mut(reference.node).filter(|seen| !**seen) {
                *seen = true;
                queue.push_back(reference.node);
            }
        }
    }
    calls
}

fn location_of(elf: &Elf<'_>, graph: &CallGraph<'_>, caller: usize, at: u32) -> Option<Location> {
    location_reference(graph.references(caller), at)
        .find_map(|reference| Location::read(elf, reference.target))
}

fn location_reference(references: &[Reference], at: u32) -> impl Iterator<Item = &Reference> {
    references
        .iter()
        .rev()
        .filter(move |reference| reference.at < at && at - reference.at <= LOCATION_WINDOW)
        .take_while(|reference| !reference.call)
}

fn is_panic_entry(name: &str) -> bool {
    name.strip_prefix("core::").is_some_and(|path| {
        path.split("::").any(|segment| {
            let segment = segment.split('<').next().unwrap_or_default();
            segment.contains("panic") || segment.ends_with("_fail") || segment.ends_with("_failed")
        })
    })
}

fn is_resumption_after_completion(name: &str) -> bool {
    name.rsplit("::").next().is_some_and(|function| {
        function.starts_with("panic_const_") && function.contains("resumed")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_entries_are_the_core_panic_functions() {
        for name in [
            "core::panicking::panic_fmt",
            "core::panicking::assert_failed::<usize, usize>",
            "core::option::unwrap_failed",
            "core::option::expect_failed",
            "core::cell::panic_already_borrowed",
            "core::slice::index::slice_end_index_len_fail",
        ] {
            assert!(is_panic_entry(name), "{name}");
        }
        for name in [
            "core::fmt::write",
            "openwoot::scan::panic_free",
            "<core::cell::RefCell<u8>>::borrow_mut",
        ] {
            assert!(!is_panic_entry(name), "{name}");
        }
    }

    #[test]
    fn the_location_is_taken_only_from_references_after_the_last_call() {
        let reference = |at, target, call| Reference {
            at,
            target,
            node: 0,
            call,
        };
        let references = [
            reference(0x100, 0x2000, false),
            reference(0x104, 0x3000, true),
            reference(0x10A, 0x4000, false),
            reference(0x110, 0x5000, false),
        ];
        let candidates = |at| {
            location_reference(&references, at)
                .map(|reference| reference.target)
                .collect::<Vec<_>>()
        };
        assert_eq!(candidates(0x114), [0x5000, 0x4000]);
        assert_eq!(candidates(0x108), Vec::<u32>::new());
        assert_eq!(candidates(0x102), [0x2000]);
        assert_eq!(candidates(0x110 + LOCATION_WINDOW + 1), Vec::<u32>::new());
    }

    #[test]
    fn only_resumption_after_completion_is_left_out() {
        assert!(is_resumption_after_completion(
            "core::panicking::panic_const::panic_const_async_fn_resumed"
        ));
        assert!(!is_resumption_after_completion(
            "core::panicking::panic_const::panic_const_div_by_zero"
        ));
    }
}
