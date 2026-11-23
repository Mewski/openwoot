use std::collections::VecDeque;

use anyhow::Result;
use openwoot_image::Hex;

use super::{call_graph::CallGraph, entry_points::EntryPoints};

pub(super) fn critical_sections(
    graph: &CallGraph<'_>,
    entry_points: &EntryPoints,
) -> Result<Vec<String>> {
    let scan = entry_points.scan;
    let mut problems = Vec::new();
    for node in reachable(graph.len(), scan, |node| {
        graph
            .references(node)
            .iter()
            .map(|reference| reference.node)
            .collect()
    }) {
        for address in graph.basepri_writes(node)? {
            problems.push(format!(
                "`{}` reaches a BASEPRI write in `{}` at {}; the scan interrupt takes no \
                 critical section",
                graph.name(scan),
                graph.name(node),
                Hex(address)
            ));
        }
    }
    Ok(problems)
}

fn reachable(len: usize, root: usize, successors: impl Fn(usize) -> Vec<usize>) -> Vec<usize> {
    let mut seen = vec![false; len];
    let mut queue = VecDeque::from([root]);
    let mut nodes = Vec::new();
    while let Some(node) = queue.pop_front() {
        let Some(visited) = seen.get_mut(node).filter(|visited| !**visited) else {
            continue;
        };
        *visited = true;
        nodes.push(node);
        queue.extend(successors(node));
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_node_the_scan_vector_reaches_is_visited_once() {
        let edges: [&[usize]; 5] = [&[1, 2], &[2, 0], &[3], &[], &[0]];
        let mut nodes = reachable(edges.len(), 0, |node| edges[node].to_vec());
        nodes.sort_unstable();
        assert_eq!(nodes, [0, 1, 2, 3]);
    }
}
