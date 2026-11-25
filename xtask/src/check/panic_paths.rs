mod location;
mod reviewed;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use location::Location;
use openwoot_image::Hex;
pub(super) use reviewed::REVIEWED;

use crate::{
    call_graph::{CallGraph, Reference},
    elf::Elf,
    entry_points::{EntryPoints, Executor},
};

const LOCATION_WINDOW: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Origin {
    Interrupt,
    Task(Executor),
}

pub(super) struct Findings {
    pub(super) problems: Vec<String>,
    pub(super) reviewed: BTreeSet<usize>,
}

struct Site {
    entry: usize,
    roots: Vec<(usize, Origin)>,
}

struct Code {
    literal_loads: Vec<(u32, u32)>,
    jumps: Vec<(u32, u32)>,
}

impl Code {
    fn read(graph: &CallGraph<'_>, function: usize) -> Option<Self> {
        Some(Self {
            literal_loads: graph.literal_loads(function).ok()?,
            jumps: graph.jumps(function).ok()?,
        })
    }
}

pub(super) fn find(elf: &Elf<'_>, graph: &CallGraph<'_>, entry_points: &EntryPoints) -> Findings {
    let roots = entry_points
        .handlers
        .iter()
        .map(|&root| (root, Origin::Interrupt))
        .chain(Executor::ALL.into_iter().flat_map(|executor| {
            entry_points
                .tasks_of(executor)
                .iter()
                .map(move |&root| (root, Origin::Task(executor)))
        }));
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
    let mut codes = BTreeMap::new();
    for ((caller, at), Site { entry, roots }) in sites {
        let code = codes
            .entry(caller)
            .or_insert_with(|| Code::read(graph, caller));
        let locations = code.as_ref().map_or_else(Vec::new, |code| {
            locations_feeding(graph.references(caller), code, at, |address| {
                Location::read(elf, address)
            })
        });
        let places = if locations.is_empty() {
            vec![(Hex(at).to_string(), None)]
        } else {
            locations
                .iter()
                .map(|location| {
                    let reviewed = REVIEWED
                        .iter()
                        .position(|reviewed| reviewed.matches(location));
                    (location.to_string(), reviewed)
                })
                .collect()
        };
        for (place, reviewed) in places {
            findings.reviewed.extend(reviewed);
            let covering = reviewed.and_then(|index| REVIEWED.get(index));
            let uncovered = roots.iter().find(|(_, origin)| {
                covering.is_none_or(|reviewed| !reviewed.reached_from.contains(origin))
            });
            let Some(&(root, _)) = uncovered else {
                continue;
            };
            let problem = format!(
                "`{}` reaches `{}` in `{}` at `{place}`",
                graph.name(root),
                graph.name(entry),
                graph.name(caller)
            );
            if !findings.problems.contains(&problem) {
                findings.problems.push(problem);
            }
        }
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

fn locations_feeding(
    references: &[Reference],
    code: &Code,
    at: u32,
    read: impl Fn(u32) -> Option<Location>,
) -> Vec<Location> {
    let mut locations = Vec::new();
    let mut visited = BTreeSet::new();
    let mut pending = VecDeque::from([at]);
    while let Some(point) = pending.pop_front() {
        if !visited.insert(point) {
            continue;
        }
        let fence = fence_before(references, &code.jumps, point);
        let found = location_reference(references, &code.literal_loads, fence, point)
            .find_map(|reference| read(reference.target))
            .filter(|location| !locations.contains(location));
        locations.extend(found);
        pending.extend(
            code.jumps
                .iter()
                .filter(|&&(_, target)| target <= point && fence.is_none_or(|fence| fence < target))
                .map(|&(jump, _)| jump),
        );
    }
    locations
}

fn location_reference<'a>(
    references: &'a [Reference],
    literal_loads: &'a [(u32, u32)],
    fence: Option<u32>,
    at: u32,
) -> impl Iterator<Item = &'a Reference> {
    let before = move |address: u32| {
        address < at && at - address <= LOCATION_WINDOW && fence.is_none_or(|fence| fence < address)
    };
    let loaded = literal_loads
        .iter()
        .rev()
        .filter(move |&&(load, _)| before(load))
        .filter_map(move |&(_, literal)| {
            references.iter().find(|reference| reference.at == literal)
        });
    references
        .iter()
        .rev()
        .filter(move |reference| before(reference.at))
        .chain(loaded)
}

fn fence_before(references: &[Reference], jumps: &[(u32, u32)], address: u32) -> Option<u32> {
    references
        .iter()
        .filter(|reference| reference.call)
        .map(|reference| reference.at)
        .chain(jumps.iter().map(|&(jump, _)| jump))
        .filter(|&fence| fence < address)
        .max()
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

    fn reference(at: u32, target: u32, call: bool) -> Reference {
        Reference {
            at,
            target,
            node: 0,
            call,
        }
    }

    fn location(address: u32) -> Option<Location> {
        (address >= 0x1000).then(|| Location {
            file: "file.rs".to_owned(),
            line: address,
            column: 1,
        })
    }

    fn candidates(references: &[Reference], jumps: &[(u32, u32)], at: u32) -> Vec<u32> {
        let fence = fence_before(references, jumps, at);
        location_reference(references, &[], fence, at)
            .map(|reference| reference.target)
            .collect()
    }

    fn feeding(references: &[Reference], jumps: &[(u32, u32)], at: u32) -> Vec<u32> {
        let code = Code {
            literal_loads: Vec::new(),
            jumps: jumps.to_vec(),
        };
        locations_feeding(references, &code, at, location)
            .into_iter()
            .map(|location| location.line)
            .collect()
    }

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
        let references = [
            reference(0x100, 0x2000, false),
            reference(0x104, 0x3000, true),
            reference(0x10A, 0x4000, false),
            reference(0x110, 0x5000, false),
        ];
        assert_eq!(candidates(&references, &[], 0x114), [0x5000, 0x4000]);
        assert_eq!(candidates(&references, &[], 0x108), Vec::<u32>::new());
        assert_eq!(candidates(&references, &[], 0x102), [0x2000]);
        assert_eq!(
            candidates(&references, &[], 0x110 + LOCATION_WINDOW + 1),
            Vec::<u32>::new()
        );
    }

    #[test]
    fn a_jump_ends_the_search_like_a_call() {
        let references = [
            reference(0x100, 0x2000, false),
            reference(0x10A, 0x4000, false),
        ];
        let jumps = [(0x104, 0x200)];
        assert_eq!(candidates(&references, &jumps, 0x10E), [0x4000]);
        assert_eq!(candidates(&references, &jumps, 0x104), [0x2000]);
        assert_eq!(candidates(&references, &[], 0x10E), [0x4000, 0x2000]);
    }

    #[test]
    fn literal_pool_loads_after_the_last_call_name_their_location() {
        let references = [
            reference(0x104, 0x3000, true),
            reference(0x200, 0x6000, false),
            reference(0x204, 0x7000, false),
        ];
        let loads = [(0x102, 0x204), (0x10C, 0x200)];
        let candidates = |at| {
            let fence = fence_before(&references, &[], at);
            location_reference(&references, &loads, fence, at)
                .map(|reference| reference.target)
                .collect::<Vec<_>>()
        };
        assert_eq!(candidates(0x110), [0x6000]);
        assert_eq!(candidates(0x103), [0x7000]);
        assert_eq!(candidates(0x108), Vec::<u32>::new());
    }

    #[test]
    fn a_call_with_one_path_into_it_has_the_location_loaded_before_it() {
        let references = [
            reference(0x0F0, 0x8000, true),
            reference(0x104, 0x2000, false),
            reference(0x120, 0x9000, true),
        ];
        assert_eq!(feeding(&references, &[], 0x120), [0x2000]);
        assert_eq!(feeding(&references, &[], 0x0F0), Vec::<u32>::new());
    }

    #[test]
    fn every_jump_into_the_block_of_a_call_adds_the_location_loaded_before_it() {
        let references = [
            reference(0x0F0, 0x8000, true),
            reference(0x104, 0x2000, false),
            reference(0x10E, 0x3000, false),
            reference(0x114, 0x4000, false),
            reference(0x120, 0x9000, true),
        ];
        let jumps = [(0x108, 0x118), (0x112, 0x118)];
        assert_eq!(
            feeding(&references, &jumps, 0x120),
            [0x4000, 0x2000, 0x3000]
        );
    }

    #[test]
    fn a_jump_to_the_call_itself_or_from_after_it_joins_the_block() {
        let references = [
            reference(0x0F0, 0x8000, true),
            reference(0x100, 0x9000, true),
            reference(0x10C, 0x2000, false),
            reference(0x114, 0x3000, false),
        ];
        let jumps = [(0x10E, 0x0F8), (0x118, 0x100)];
        assert_eq!(feeding(&references, &jumps, 0x100), [0x2000, 0x3000]);
    }

    #[test]
    fn a_jump_outside_the_block_of_a_call_adds_nothing() {
        let references = [
            reference(0x0F0, 0x8000, true),
            reference(0x0F8, 0x6000, false),
            reference(0x104, 0x2000, false),
            reference(0x10E, 0x3000, false),
            reference(0x120, 0x9000, true),
        ];
        let inside = (0x0FC, 0x118);
        assert_eq!(feeding(&references, &[inside], 0x120), [0x3000, 0x6000]);
        let before_the_previous_call = (0x0FC, 0x0E0);
        let at_the_previous_call = (0x0FC, 0x0F0);
        let before_the_last_jump = (0x0FC, 0x0F8);
        let beyond_the_call = (0x0FC, 0x124);
        for jump in [
            before_the_previous_call,
            at_the_previous_call,
            before_the_last_jump,
            beyond_the_call,
        ] {
            assert_eq!(feeding(&references, &[jump], 0x120), [0x3000]);
        }
    }

    #[test]
    fn a_jump_into_a_long_block_still_joins_it() {
        let references = [
            reference(0x0F0, 0x2000, false),
            reference(0x200, 0x9000, true),
        ];
        let jumps = [(0x100, 0x108)];
        assert_eq!(feeding(&references, &jumps, 0x200), [0x2000]);
    }

    #[test]
    fn a_location_loaded_for_several_jumps_is_listed_once() {
        let references = [
            reference(0x0F0, 0x8000, true),
            reference(0x104, 0x2000, false),
            reference(0x112, 0x2000, false),
            reference(0x120, 0x9000, true),
        ];
        let jumps = [(0x108, 0x118), (0x116, 0x118)];
        assert_eq!(feeding(&references, &jumps, 0x120), [0x2000]);
    }

    #[test]
    fn a_reference_that_names_no_location_is_passed_over() {
        let references = [
            reference(0x0F0, 0x8000, true),
            reference(0x100, 0x2000, false),
            reference(0x104, 0x0010, false),
            reference(0x120, 0x9000, true),
        ];
        assert_eq!(feeding(&references, &[(0x108, 0x118)], 0x120), [0x2000]);
    }

    #[test]
    fn a_literal_pool_location_before_a_jump_is_found() {
        let references = [
            reference(0x0F0, 0x8000, true),
            reference(0x120, 0x9000, true),
            reference(0x200, 0x2000, false),
            reference(0x204, 0x3000, false),
        ];
        let code = Code {
            literal_loads: vec![(0x102, 0x204), (0x10A, 0x200)],
            jumps: vec![(0x106, 0x118)],
        };
        let lines: Vec<u32> = locations_feeding(&references, &code, 0x120, location)
            .into_iter()
            .map(|location| location.line)
            .collect();
        assert_eq!(lines, [0x2000, 0x3000]);
    }

    #[test]
    fn jumps_that_lead_into_each_other_are_followed_once() {
        let references = [
            reference(0x0F8, 0x2000, false),
            reference(0x10C, 0x3000, false),
        ];
        let jumps = [(0x108, 0x110), (0x118, 0x100)];
        assert_eq!(feeding(&references, &jumps, 0x118), [0x3000, 0x2000]);
    }

    fn merged_pads() -> ([Reference; 12], [(u32, u32); 4]) {
        let references = [
            reference(0x0C0, 0x8000, true),
            reference(0x0F6, 0x6000, false),
            reference(0x10E, 0x5000, false),
            reference(0x11A, 0x0010, false),
            reference(0x12A, 0x9000, true),
            reference(0x1F7A, 0x8000, true),
            reference(0x1F82, 0x2000, false),
            reference(0x1F8C, 0x3000, false),
            reference(0x1F96, 0x4000, false),
            reference(0x1FA6, 0x0010, false),
            reference(0x1FB2, 0x7000, false),
            reference(0x1FB6, 0x9100, true),
        ];
        let jumps = [
            (0x0FA, 0x112),
            (0x1F86, 0x1F9A),
            (0x1F90, 0x1F9A),
            (0x1FAA, 0x122),
        ];
        (references, jumps)
    }

    #[test]
    fn pads_merged_in_several_levels_are_all_found() {
        let (references, jumps) = merged_pads();
        assert_eq!(
            feeding(&references, &jumps, 0x12A),
            [0x5000, 0x6000, 0x4000, 0x2000, 0x3000]
        );
    }

    #[test]
    fn a_call_after_a_jump_does_not_inherit_the_pads_before_it() {
        let (references, jumps) = merged_pads();
        assert_eq!(feeding(&references, &jumps, 0x1FB6), [0x7000]);
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
