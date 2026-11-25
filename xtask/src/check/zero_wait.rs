use std::collections::BTreeMap;

use openwoot_image::{Hex, Region};

use crate::{call_graph::CallGraph, elf::Symbol, entry_points::EntryPoints};

pub(super) fn code_beyond(
    graph: &CallGraph<'_>,
    entry_points: &EntryPoints,
    zero_wait: Region,
) -> Vec<String> {
    let mut reached_from = BTreeMap::new();
    for &root in entry_points
        .interrupts
        .iter()
        .chain(&entry_points.key_tasks)
    {
        for node in graph.reachable([root]) {
            reached_from.entry(node).or_insert(root);
        }
    }
    reached_from
        .into_iter()
        .filter(|&(node, _)| graph.is_function(node))
        .filter_map(|(node, root)| {
            let function = graph.symbol(node)?;
            outside(zero_wait, function).then(|| problem(graph.name(root), function, zero_wait))
        })
        .collect()
}

fn outside(zero_wait: Region, function: &Symbol) -> bool {
    function.address < zero_wait.start
        || function.address.saturating_add(function.size) > zero_wait.end
}

fn problem(root: &str, function: &Symbol, zero_wait: Region) -> String {
    format!(
        "`{root}` runs `{}` at {}, beyond the zero-wait flash that ends at {} (`ZERO_WAIT` in \
         `memory.x`)",
        function.name,
        Hex(function.address),
        Hex(zero_wait.end)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elf::SymbolKind;

    const ZERO_WAIT: Region = Region {
        start: 0x0800_0000,
        end: 0x0808_0000,
    };

    fn function(address: u32, size: u32) -> Symbol {
        Symbol {
            name: "sweep_step".to_owned(),
            link_name: "_RNvsweep_step".to_owned(),
            address,
            size,
            kind: SymbolKind::Function,
        }
    }

    #[test]
    fn a_function_that_ends_at_the_zero_wait_end_lies_inside() {
        assert!(!outside(ZERO_WAIT, &function(0x0807_FF00, 0x100)));
        assert!(!outside(ZERO_WAIT, &function(0x0800_8000, 0x40)));
    }

    #[test]
    fn a_function_that_crosses_or_lies_beyond_the_end_lies_outside() {
        assert!(outside(ZERO_WAIT, &function(0x0807_FF00, 0x101)));
        assert!(outside(ZERO_WAIT, &function(0x0808_5D60, 0xC2A)));
        assert!(outside(ZERO_WAIT, &function(0x07FF_FFF0, 0x20)));
    }

    #[test]
    fn the_problem_names_the_root_the_function_and_the_end() {
        assert_eq!(
            problem("ADC", &function(0x0808_5D60, 0xC2A), ZERO_WAIT),
            "`ADC` runs `sweep_step` at 0x0808_5d60, beyond the zero-wait flash that ends at \
             0x0808_0000 (`ZERO_WAIT` in `memory.x`)"
        );
    }
}
