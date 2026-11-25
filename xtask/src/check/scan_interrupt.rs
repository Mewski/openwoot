use anyhow::Result;
use openwoot_image::Hex;

use crate::{call_graph::CallGraph, entry_points::EntryPoints};

pub(super) fn critical_sections(
    graph: &CallGraph<'_>,
    entry_points: &EntryPoints,
) -> Result<Vec<String>> {
    let scan = entry_points.scan;
    let mut problems = Vec::new();
    for node in graph.reachable([scan]) {
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
