mod thumb;

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use openwoot_image::Hex;

use crate::elf::{Elf, RelocationKind, Symbol, SymbolKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Reference {
    pub(super) at: u32,
    pub(super) target: u32,
    pub(super) node: usize,
}

pub(super) struct CallGraph<'a> {
    elf: &'a Elf<'a>,
    nodes: Vec<Symbol>,
    markers: Vec<Symbol>,
    references: Vec<Vec<Reference>>,
}

impl<'a> CallGraph<'a> {
    pub(super) fn new(elf: &'a Elf<'a>) -> Result<Self> {
        let (mut markers, mut nodes): (Vec<Symbol>, Vec<Symbol>) =
            elf.symbols()?.into_iter().partition(Symbol::is_marker);
        nodes.extend(elf.allocated_sections().iter().map(|section| Symbol {
            name: section.name.clone(),
            address: section.address,
            size: 0,
            kind: SymbolKind::Object,
        }));
        nodes.sort_by(|left, right| {
            left.address
                .cmp(&right.address)
                .then(right.size.cmp(&left.size))
        });
        nodes.dedup_by_key(|node| node.address);
        markers.sort_by_key(|marker| marker.address);
        let mut graph = Self {
            elf,
            references: vec![Vec::new(); nodes.len()],
            nodes,
            markers,
        };
        graph.link()?;
        Ok(graph)
    }

    pub(super) fn len(&self) -> usize {
        self.nodes.len()
    }

    pub(super) fn symbol(&self, node: usize) -> Option<&Symbol> {
        self.nodes.get(node)
    }

    pub(super) fn name(&self, node: usize) -> &str {
        self.nodes
            .get(node)
            .map_or("", |symbol| symbol.name.as_str())
    }

    pub(super) fn references(&self, node: usize) -> &[Reference] {
        self.references.get(node).map_or(&[], Vec::as_slice)
    }

    pub(super) fn nodes_named(&self, matches: impl Fn(&str) -> bool) -> Vec<usize> {
        (0..self.nodes.len())
            .filter(|&node| matches(self.name(node)))
            .collect()
    }

    pub(super) fn node_at(&self, address: u32) -> Option<usize> {
        if !self
            .elf
            .allocated_sections()
            .iter()
            .any(|section| address >= section.address && address - section.address < section.size)
        {
            return None;
        }
        self.nodes
            .partition_point(|node| node.address <= address)
            .checked_sub(1)
    }

    pub(super) fn is_function(&self, node: usize) -> bool {
        self.symbol(node)
            .is_some_and(|symbol| symbol.kind == SymbolKind::Function)
    }

    pub(super) fn frame(&self, node: usize) -> Result<u32> {
        let Some(symbol) = self.symbol(node).filter(|_| self.is_function(node)) else {
            return Ok(0);
        };
        let code = self
            .elf
            .bytes_at(symbol.address, usize::try_from(symbol.size)?)
            .with_context(|| format!("the code of `{}` lies outside the image", symbol.name))?;
        let mut frame = 0_u32;
        let mut offset = 0_usize;
        while let Some(rest) = code.get(offset..) {
            let address = symbol.address + u32::try_from(offset)?;
            if let Some(code_start) = self.literals_end(address, symbol) {
                offset = usize::try_from(code_start - symbol.address)?;
                continue;
            }
            let Some(first) = rest.first_chunk().map(|bytes| u16::from_le_bytes(*bytes)) else {
                break;
            };
            let change = if thumb::is_wide(first) {
                offset += 4;
                thumb::halves(rest).and_then(thumb::wide_stack_change)
            } else {
                offset += 2;
                thumb::narrow_stack_change(first)
            };
            match change {
                Some(thumb::StackChange::Grows(bytes)) => frame = frame.saturating_add(bytes),
                Some(thumb::StackChange::Unknown) => bail!(
                    "`{}` moves the stack pointer by a register at {}",
                    symbol.name,
                    Hex(address)
                ),
                None => {}
            }
        }
        Ok(frame)
    }

    fn literals_end(&self, address: u32, function: &Symbol) -> Option<u32> {
        let index = self
            .markers
            .partition_point(|marker| marker.address <= address)
            .checked_sub(1)?;
        let marker = self.markers.get(index)?;
        if marker.kind != SymbolKind::LiteralsStart || !function.contains(marker.address) {
            return None;
        }
        let end = self
            .markers
            .get(index + 1)
            .map_or(function.address + function.size, |next| next.address)
            .min(function.address + function.size);
        (end > address).then_some(end)
    }

    fn link(&mut self) -> Result<()> {
        let mut low_halves: HashMap<(usize, u8), u16> = HashMap::new();
        for relocation in self.elf.relocations()? {
            let at = relocation.offset;
            let Some(source) = self.node_at(at) else {
                continue;
            };
            let instruction = || {
                self.elf
                    .bytes_at(at, 4)
                    .and_then(thumb::halves)
                    .with_context(|| format!("the relocated instruction at {} is missing", Hex(at)))
            };
            let target = match relocation.kind {
                RelocationKind::Absolute => self
                    .elf
                    .word_at(at)
                    .with_context(|| format!("the relocated word at {} is missing", Hex(at)))?,
                RelocationKind::Branch => thumb::branch_target(at, instruction()?),
                RelocationKind::ConditionalBranch => {
                    thumb::conditional_branch_target(at, instruction()?)
                }
                RelocationKind::MoveLow => {
                    let (register, low) = thumb::move_immediate(instruction()?);
                    low_halves.insert((source, register), low);
                    continue;
                }
                RelocationKind::MoveHigh => {
                    let (register, high) = thumb::move_immediate(instruction()?);
                    let Some(low) = low_halves.remove(&(source, register)) else {
                        bail!(
                            "the upper half of an address at {} has no lower half",
                            Hex(at)
                        );
                    };
                    u32::from(high) << 16 | u32::from(low)
                }
            };
            let code = target & !1;
            let node = match self.node_at(code) {
                Some(node) if self.is_function(node) => Some(node),
                _ => self.node_at(target),
            };
            if let (Some(node), Some(references)) = (node, self.references.get_mut(source)) {
                references.push(Reference { at, target, node });
            }
        }
        Ok(())
    }
}
