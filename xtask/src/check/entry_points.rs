use anyhow::{Context, Result, bail, ensure};

use super::call_graph::CallGraph;
use crate::elf::Elf;

const KEY_TASK: &str = "scanning";
const TASK_POLL_PREFIX: &str = "<embassy_executor::raw::TaskStorage<";
const TASK_POLL_SUFFIX: &str = ">>::poll";
const RESET_VECTOR: usize = 1;
const SCAN_VECTORS: [&str; 2] = ["ADC", "ADC0_1"];

pub(super) struct EntryPoints {
    pub(super) reset: usize,
    pub(super) handlers: Vec<usize>,
    pub(super) scan: usize,
    pub(super) key_tasks: Vec<usize>,
    pub(super) thread_tasks: Vec<usize>,
}

impl EntryPoints {
    pub(super) fn find(elf: &Elf<'_>, graph: &CallGraph<'_>) -> Result<Self> {
        let vectors = elf
            .section(".vector_table")
            .context("the ELF file has no `.vector_table` section")?;
        let mut reset = None;
        let mut handlers = Vec::new();
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
            } else if !handlers.contains(&node) {
                handlers.push(node);
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
        let scan = handlers
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
            scan,
            key_tasks,
            thread_tasks,
        })
    }
}
