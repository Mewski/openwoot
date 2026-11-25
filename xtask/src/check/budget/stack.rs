mod components;

use std::collections::HashMap;

use anyhow::{Result, bail};
use components::Components;

use crate::{call_graph::CallGraph, entry_points::EntryPoints};

const EXCEPTION_FRAME: u32 = 108;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::check) struct StackBound {
    pub(in crate::check) thread: u32,
    pub(in crate::check) interrupts: u32,
}

impl StackBound {
    pub(in crate::check) const fn total(self) -> u32 {
        self.thread.saturating_add(self.interrupts)
    }
}

pub(super) fn bound(graph: &CallGraph<'_>, entry_points: &EntryPoints) -> Result<StackBound> {
    let calls = Components::new(graph.len(), |node| {
        graph
            .references(node)
            .iter()
            .filter(|reference| reference.call)
            .map(|reference| reference.node)
            .collect()
    });
    without_recursion(&calls, |node| graph.name(node).to_owned())?;
    let components = Components::new(graph.len(), |node| {
        graph
            .references(node)
            .iter()
            .map(|reference| reference.node)
            .collect()
    });
    let mut depths = Depths::new(&components, |node| graph.frame(node));
    let thread_tasks = depths.deepest(&entry_points.thread_tasks)?;
    let key_tasks = depths.deepest(&entry_points.key_tasks)?;
    let start_up = depths.depth(entry_points.reset)?;
    let executor = entry_points
        .thread_executors
        .iter()
        .copied()
        .try_fold(None, |deepest: Option<u32>, executor| {
            let reach = depths.reach(entry_points.reset, executor)?;
            Ok::<_, anyhow::Error>(match (deepest, reach) {
                (Some(deepest), Some(reach)) => Some(deepest.max(reach)),
                (deepest, reach) => deepest.or(reach),
            })
        })?
        .unwrap_or(start_up);
    let mut interrupts = key_tasks;
    for &handler in &entry_points.handlers {
        interrupts = interrupts
            .saturating_add(depths.depth(handler)?)
            .saturating_add(EXCEPTION_FRAME);
    }
    Ok(StackBound {
        thread: start_up.max(executor.saturating_add(thread_tasks)),
        interrupts,
    })
}

fn without_recursion(calls: &Components, name: impl Fn(usize) -> String) -> Result<()> {
    if let Some(cycle) = calls.cycles().next() {
        let names: Vec<String> = cycle
            .iter()
            .map(|&node| format!("`{}`", name(node)))
            .collect();
        bail!(
            "the calls of {} form a cycle; the stack bound needs code without recursion",
            names.join(", ")
        );
    }
    Ok(())
}

struct Depths<'c, Frame> {
    components: &'c Components,
    frame: Frame,
    costs: HashMap<usize, u32>,
    memo: HashMap<usize, u32>,
}

impl<'c, Frame: FnMut(usize) -> Result<u32>> Depths<'c, Frame> {
    fn new(components: &'c Components, frame: Frame) -> Self {
        Self {
            components,
            frame,
            costs: HashMap::new(),
            memo: HashMap::new(),
        }
    }

    fn deepest(&mut self, nodes: &[usize]) -> Result<u32> {
        nodes
            .iter()
            .try_fold(0, |deepest, &node| Ok(deepest.max(self.depth(node)?)))
    }

    fn depth(&mut self, node: usize) -> Result<u32> {
        self.component_depth(self.components.of(node))
    }

    fn component_depth(&mut self, component: usize) -> Result<u32> {
        if let Some(&depth) = self.memo.get(&component) {
            return Ok(depth);
        }
        let components = self.components;
        let mut deepest = 0;
        for &successor in components.successors(component) {
            deepest = deepest.max(self.component_depth(successor)?);
        }
        let depth = self.cost(component)?.saturating_add(deepest);
        self.memo.insert(component, depth);
        Ok(depth)
    }

    fn cost(&mut self, component: usize) -> Result<u32> {
        if let Some(&cost) = self.costs.get(&component) {
            return Ok(cost);
        }
        let components = self.components;
        let mut cost = 0_u32;
        for &member in components.members(component) {
            cost = cost.saturating_add((self.frame)(member)?);
        }
        self.costs.insert(component, cost);
        Ok(cost)
    }

    fn reach(&mut self, from: usize, to: usize) -> Result<Option<u32>> {
        let mut reaches = HashMap::new();
        let (start, target) = (self.components.of(from), self.components.of(to));
        self.reach_from(start, target, &mut reaches)
    }

    fn reach_from(
        &mut self,
        component: usize,
        target: usize,
        reaches: &mut HashMap<usize, Option<u32>>,
    ) -> Result<Option<u32>> {
        if component == target {
            return Ok(Some(self.cost(component)?));
        }
        if let Some(&reach) = reaches.get(&component) {
            return Ok(reach);
        }
        let components = self.components;
        let mut deepest: Option<u32> = None;
        for &successor in components.successors(component) {
            if let Some(reach) = self.reach_from(successor, target, reaches)? {
                deepest = Some(deepest.map_or(reach, |deepest| deepest.max(reach)));
            }
        }
        let reach = match deepest {
            Some(deepest) => Some(self.cost(component)?.saturating_add(deepest)),
            None => None,
        };
        reaches.insert(component, reach);
        Ok(reach)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EDGES: [&[usize]; 5] = [&[1], &[2], &[1, 3], &[], &[2]];
    const FRAMES: [u32; 5] = [8, 16, 0, 32, 4];

    fn components() -> Components {
        Components::new(EDGES.len(), |node| EDGES[node].to_vec())
    }

    #[expect(
        clippy::unnecessary_wraps,
        reason = "`Depths` takes the same fallible frame function the call graph provides"
    )]
    fn frame(node: usize) -> Result<u32> {
        Ok(FRAMES[node])
    }

    #[test]
    fn a_cycle_through_data_counts_each_function_once_whatever_the_order() {
        let components = components();
        let mut forward = Depths::new(&components, frame);
        let first = forward.depth(0).unwrap();
        let second = forward.depth(4).unwrap();
        let mut backward = Depths::new(&components, frame);
        assert_eq!(backward.depth(4).unwrap(), second);
        assert_eq!(backward.depth(0).unwrap(), first);
        assert_eq!((first, second), (8 + 16 + 32, 4 + 16 + 32));
    }

    #[test]
    fn reach_sums_the_frames_on_the_deepest_path_to_its_target() {
        let components = components();
        let mut depths = Depths::new(&components, frame);
        assert_eq!(depths.reach(0, 3).unwrap(), Some(8 + 16 + 32));
        assert_eq!(depths.reach(0, 1).unwrap(), Some(8 + 16));
        assert_eq!(depths.reach(0, 4).unwrap(), None);
    }

    #[test]
    fn a_frame_that_cannot_be_read_is_an_error() {
        let components = components();
        let mut depths = Depths::new(&components, |node: usize| -> Result<u32> {
            if node == 3 {
                bail!("`wake` moves the stack pointer by a register at 0x0800_0000");
            }
            Ok(FRAMES[node])
        });
        assert!(depths.depth(0).is_err());
    }

    #[test]
    fn calls_in_a_cycle_are_rejected_with_their_names() {
        let names = ["ping", "pong", "leaf"];
        let calls = Components::new(3, |node| [vec![1], vec![0, 2], vec![]][node].clone());
        assert_eq!(
            without_recursion(&calls, |node| names[node].to_owned())
                .unwrap_err()
                .to_string(),
            "the calls of `ping`, `pong` form a cycle; the stack bound needs code without recursion"
        );
        let calls = Components::new(3, |node| [vec![1], vec![2], vec![]][node].clone());
        assert!(without_recursion(&calls, |node| names[node].to_owned()).is_ok());
    }
}
