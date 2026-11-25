use anyhow::{Context, Result, bail, ensure};

use crate::{call_graph::CallGraph, elf::Elf};

const KEY_TASK: &str = "scanning";
const TASK_POLL_PREFIX: &str = "<embassy_executor::raw::TaskStorage<";
const TASK_POLL_SUFFIX: &str = ">>::poll";
const THREAD_EXECUTOR: &str = "<embassy_executor::platform::thread::Executor>::run";
const RESET_VECTOR: usize = 1;
const FIRST_DEVICE_VECTOR: usize = 16;
const SCAN_VECTORS: [&str; 2] = ["ADC", "ADC0_1"];

pub(crate) struct EntryPoints {
    pub(crate) reset: usize,
    pub(crate) handlers: Vec<usize>,
    pub(crate) interrupts: Vec<usize>,
    pub(crate) scan: usize,
    pub(crate) key_tasks: Vec<usize>,
    pub(crate) thread_executors: Vec<usize>,
    pub(crate) thread_tasks: Vec<usize>,
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
        let key_marker = format!("::__{KEY_TASK}_task::");
        let tasks = graph.nodes_named(|name| {
            name.starts_with(TASK_POLL_PREFIX) && name.ends_with(TASK_POLL_SUFFIX)
        });
        let (key_tasks, thread_tasks): (Vec<usize>, Vec<usize>) = tasks
            .into_iter()
            .partition(|&node| graph.name(node).contains(&key_marker));
        if key_tasks.is_empty() {
            bail!("the image has no poll function of the key task `{KEY_TASK}`");
        }
        Ok(Self {
            reset,
            handlers,
            interrupts,
            scan,
            key_tasks,
            thread_executors: graph.nodes_named(|name| name.starts_with(THREAD_EXECUTOR)),
            thread_tasks,
        })
    }
}
