use anyhow::{Context, Result, bail, ensure};

use crate::{call_graph::CallGraph, elf::Elf};

const TASK_POLL_PREFIX: &str = "<embassy_executor::raw::TaskStorage<";
const TASK_POLL_SUFFIX: &str = ">>::poll";
const THREAD_EXECUTOR: &str = "<embassy_executor::platform::thread::Executor>::run";
const RESET_VECTOR: usize = 1;
const FIRST_DEVICE_VECTOR: usize = 16;
const SCAN_VECTORS: [&str; 2] = ["ADC", "ADC0_1"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Executor {
    Keys,
    Device,
    Lighting,
}

pub(crate) struct EntryPoints {
    pub(crate) reset: usize,
    pub(crate) handlers: Vec<usize>,
    pub(crate) interrupts: Vec<usize>,
    pub(crate) scan: usize,
    pub(crate) interrupt_tasks: Vec<(Executor, Vec<usize>)>,
    pub(crate) thread_executors: Vec<usize>,
    pub(crate) thread_tasks: Vec<usize>,
}

impl Executor {
    pub(crate) const ALL: [Self; 3] = [Self::Keys, Self::Device, Self::Lighting];

    pub(crate) const fn tasks(self) -> &'static [&'static str] {
        match self {
            Self::Keys => &["scanning"],
            Self::Device => &["device", "reports"],
            Self::Lighting => &["lights"],
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Keys => "keys",
            Self::Device => "device",
            Self::Lighting => "lighting",
        }
    }

    pub(crate) const fn runs_for_speed(self) -> bool {
        matches!(self, Self::Keys | Self::Device)
    }

    fn of(task: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|executor| {
            executor
                .tasks()
                .iter()
                .any(|name| task.contains(&format!("::__{name}_task::")))
        })
    }
}

impl EntryPoints {
    pub(crate) fn find(elf: &Elf<'_>, graph: &CallGraph<'_>) -> Result<Self> {
        let vectors = elf
            .section(".vector_table")
            .context("the ELF file has no `.vector_table` section")?;
        let mut reset = None;
        let mut handlers = Vec::new();
        let mut interrupts = Vec::new();
        let (vectors, _) = vectors.bytes.as_chunks::<4>();
        for (index, vector) in vectors.iter().enumerate().skip(RESET_VECTOR) {
            let address = u32::from_le_bytes(*vector);
            if address == 0 {
                continue;
            }
            let node = graph
                .node_at(address & !1)
                .filter(|&node| graph.is_function(node))
                .with_context(|| format!("vector {index} points outside the code"))?;
            if index == RESET_VECTOR {
                reset = Some(node);
                continue;
            }
            if !handlers.contains(&node) {
                handlers.push(node);
            }
            if index >= FIRST_DEVICE_VECTOR && !interrupts.contains(&node) {
                interrupts.push(node);
            }
        }
        let reset = reset.context("the vector table has no reset vector")?;
        ensure!(
            graph
                .references(reset)
                .iter()
                .any(|reference| reference.call),
            "the reset handler `{}` calls nothing in the relocations; boards link with \
             `--emit-relocs`",
            graph.name(reset)
        );
        let scan = interrupts
            .iter()
            .copied()
            .find(|&node| SCAN_VECTORS.contains(&graph.name(node)))
            .with_context(|| {
                format!(
                    "the vector table has no scan interrupt handler `{}`",
                    SCAN_VECTORS.join("` or `")
                )
            })?;
        let mut interrupt_tasks: Vec<(Executor, Vec<usize>)> = Executor::ALL
            .into_iter()
            .map(|executor| (executor, Vec::new()))
            .collect();
        let mut thread_tasks = Vec::new();
        for task in graph.nodes_named(|name| {
            name.starts_with(TASK_POLL_PREFIX) && name.ends_with(TASK_POLL_SUFFIX)
        }) {
            let executor = Executor::of(graph.name(task));
            match interrupt_tasks
                .iter_mut()
                .find(|(known, _)| Some(*known) == executor)
            {
                Some((_, tasks)) => tasks.push(task),
                None => thread_tasks.push(task),
            }
        }
        for (executor, tasks) in &interrupt_tasks {
            for name in executor.tasks() {
                let marker = format!("::__{name}_task::");
                if !tasks.iter().any(|&task| graph.name(task).contains(&marker)) {
                    bail!(
                        "the image has no poll function of the task `{name}` of the {} executor",
                        executor.name()
                    );
                }
            }
        }
        Ok(Self {
            reset,
            handlers,
            interrupts,
            scan,
            interrupt_tasks,
            thread_executors: graph.nodes_named(|name| name.starts_with(THREAD_EXECUTOR)),
            thread_tasks,
        })
    }

    pub(crate) fn tasks_of(&self, executor: Executor) -> &[usize] {
        self.interrupt_tasks
            .iter()
            .find(|(known, _)| *known == executor)
            .map_or(&[], |(_, tasks)| tasks.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_belongs_to_the_executor_that_names_it() {
        let poll =
            |path: &str| format!("{TASK_POLL_PREFIX}{path}::{{closure#0}}{TASK_POLL_SUFFIX}");
        assert_eq!(
            Executor::of(&poll(
                "openwoot_wooting_80he::__scanning_task::__scanning_task_inner_function"
            )),
            Some(Executor::Keys)
        );
        assert_eq!(
            Executor::of(&poll(
                "openwoot_wooting_80he::__reports_task::__reports_task_inner_function"
            )),
            Some(Executor::Device)
        );
        assert_eq!(
            Executor::of(&poll(
                "wooting_80he::configuration::__lights_task::__lights_task_inner_function"
            )),
            Some(Executor::Lighting)
        );
        assert_eq!(
            Executor::of(&poll(
                "wooting_80he::configuration::__channel_task::__channel_task_inner_function"
            )),
            None
        );
    }
}
