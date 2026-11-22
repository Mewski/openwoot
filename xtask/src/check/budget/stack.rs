use std::collections::HashMap;

use anyhow::Result;

use crate::check::{call_graph::CallGraph, entry_points::EntryPoints};

const EXCEPTION_FRAME: u32 = 108;
const THREAD_EXECUTOR: &str = "<embassy_executor::platform::thread::Executor>::run";

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
    let mut depths = Depths {
        graph,
        frames: HashMap::new(),
        memo: HashMap::new(),
        open: vec![false; graph.len()],
    };
    let deepest_task = |depths: &mut Depths<'_, '_>, tasks: &[usize]| -> Result<u32> {
        tasks
            .iter()
            .try_fold(0, |deepest, &task| Ok(deepest.max(depths.depth(task)?)))
    };
    let thread_tasks = deepest_task(&mut depths, &entry_points.thread_tasks)?;
    let key_tasks = deepest_task(&mut depths, &entry_points.key_tasks)?;
    let start_up = depths.depth(entry_points.reset)?;
    let executor = graph
        .nodes_named(|name| name.starts_with(THREAD_EXECUTOR))
        .into_iter()
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

struct Depths<'g, 'e> {
    graph: &'g CallGraph<'e>,
    frames: HashMap<usize, u32>,
    memo: HashMap<usize, u32>,
    open: Vec<bool>,
}

impl Depths<'_, '_> {
    fn frame(&mut self, node: usize) -> Result<u32> {
        if let Some(&frame) = self.frames.get(&node) {
            return Ok(frame);
        }
        let frame = self.graph.frame(node)?;
        self.frames.insert(node, frame);
        Ok(frame)
    }

    fn depth(&mut self, node: usize) -> Result<u32> {
        if let Some(&depth) = self.memo.get(&node) {
            return Ok(depth);
        }
        if self.open.get(node).copied().unwrap_or(true) {
            return Ok(0);
        }
        self.set_open(node, true);
        let mut deepest = 0;
        for reference in self.graph.references(node) {
            deepest = deepest.max(self.depth(reference.node)?);
        }
        self.set_open(node, false);
        let depth = self.frame(node)?.saturating_add(deepest);
        self.memo.insert(node, depth);
        Ok(depth)
    }

    fn reach(&mut self, from: usize, to: usize) -> Result<Option<u32>> {
        let mut reaches = HashMap::new();
        let mut open = vec![false; self.graph.len()];
        self.reach_from(from, to, &mut reaches, &mut open)
    }

    fn reach_from(
        &mut self,
        node: usize,
        to: usize,
        reaches: &mut HashMap<usize, Option<u32>>,
        open: &mut Vec<bool>,
    ) -> Result<Option<u32>> {
        if node == to {
            return Ok(Some(self.frame(node)?));
        }
        if let Some(&reach) = reaches.get(&node) {
            return Ok(reach);
        }
        if open.get(node).copied().unwrap_or(true) {
            return Ok(None);
        }
        if let Some(entry) = open.get_mut(node) {
            *entry = true;
        }
        let mut deepest: Option<u32> = None;
        for reference in self.graph.references(node) {
            if let Some(reach) = self.reach_from(reference.node, to, reaches, open)? {
                deepest = Some(deepest.map_or(reach, |deepest| deepest.max(reach)));
            }
        }
        if let Some(entry) = open.get_mut(node) {
            *entry = false;
        }
        let reach = match deepest {
            Some(deepest) => Some(self.frame(node)?.saturating_add(deepest)),
            None => None,
        };
        reaches.insert(node, reach);
        Ok(reach)
    }

    fn set_open(&mut self, node: usize, open: bool) {
        if let Some(entry) = self.open.get_mut(node) {
            *entry = open;
        }
    }
}
