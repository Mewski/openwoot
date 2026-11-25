mod halves;
mod thumb;

use std::collections::{BTreeMap, HashSet, VecDeque};

use anyhow::{Context, Result, bail, ensure};
use halves::Half;
use openwoot_image::Hex;
use thumb::Instruction;

use crate::{
    elf::{Elf, RelocationKind, Symbol, SymbolKind},
    link_map::InputSection,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reference {
    pub(crate) at: u32,
    pub(crate) target: u32,
    pub(crate) node: usize,
    pub(crate) call: bool,
}

pub(crate) struct CallGraph<'a> {
    elf: &'a Elf<'a>,
    nodes: Vec<Symbol>,
    markers: Vec<Symbol>,
    references: Vec<Vec<Reference>>,
}

impl<'a> CallGraph<'a> {
    pub(crate) fn new(elf: &'a Elf<'a>, input_sections: &[InputSection]) -> Result<Self> {
        let (mut markers, mut nodes): (Vec<Symbol>, Vec<Symbol>) =
            elf.symbols()?.into_iter().partition(Symbol::is_marker);
        nodes.extend(elf.allocated_sections().iter().map(|section| Symbol {
            name: section.name.clone(),
            address: section.address,
            size: 0,
            kind: SymbolKind::Object,
        }));
        let nodes = with_unnamed(
            nodes,
            input_sections
                .iter()
                .filter(|section| elf.section_at(section.address).is_some()),
        );
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

    pub(crate) fn len(&self) -> usize {
        self.nodes.len()
    }

    pub(crate) fn symbol(&self, node: usize) -> Option<&Symbol> {
        self.nodes.get(node)
    }

    pub(crate) fn name(&self, node: usize) -> &str {
        self.nodes
            .get(node)
            .map_or("", |symbol| symbol.name.as_str())
    }

    pub(crate) fn references(&self, node: usize) -> &[Reference] {
        self.references.get(node).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn nodes_named(&self, matches: impl Fn(&str) -> bool) -> Vec<usize> {
        (0..self.nodes.len())
            .filter(|&node| matches(self.name(node)))
            .collect()
    }

    pub(crate) fn node_at(&self, address: u32) -> Option<usize> {
        self.elf.section_at(address)?;
        self.nodes
            .partition_point(|node| node.address <= address)
            .checked_sub(1)
    }

    pub(crate) fn is_function(&self, node: usize) -> bool {
        self.symbol(node)
            .is_some_and(|symbol| symbol.kind == SymbolKind::Function)
    }

    pub(crate) fn frame(&self, node: usize) -> Result<u32> {
        let Some(symbol) = self.symbol(node).filter(|_| self.is_function(node)) else {
            return Ok(0);
        };
        let mut frame = 0_u32;
        for (address, instruction) in self.instructions(symbol)? {
            match instruction.stack_change() {
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

    pub(crate) fn basepri_writes(&self, node: usize) -> Result<Vec<u32>> {
        let Some(symbol) = self.symbol(node).filter(|_| self.is_function(node)) else {
            return Ok(Vec::new());
        };
        Ok(self
            .instructions(symbol)?
            .into_iter()
            .filter(|(_, instruction)| instruction.writes_basepri())
            .map(|(address, _)| address)
            .collect())
    }

    pub(crate) fn literal_loads(&self, node: usize) -> Result<Vec<(u32, u32)>> {
        let Some(symbol) = self.symbol(node).filter(|_| self.is_function(node)) else {
            return Ok(Vec::new());
        };
        Ok(self
            .instructions(symbol)?
            .into_iter()
            .filter_map(|(address, instruction)| {
                instruction
                    .literal(address)
                    .map(|literal| (address, literal))
            })
            .collect())
    }

    pub(crate) fn jumps(&self, node: usize) -> Result<Vec<(u32, u32)>> {
        let Some(symbol) = self.symbol(node).filter(|_| self.is_function(node)) else {
            return Ok(Vec::new());
        };
        Ok(jumps_within(symbol, self.instructions(symbol)?))
    }

    pub(crate) fn reachable(&self, roots: impl IntoIterator<Item = usize>) -> Vec<usize> {
        reachable(self.len(), roots, |node| {
            self.references(node)
                .iter()
                .map(|reference| reference.node)
                .collect()
        })
    }

    fn instructions(&self, function: &Symbol) -> Result<Vec<(u32, Instruction)>> {
        ensure!(
            function.size > 0,
            "`{}` has no size, so its code cannot be read",
            function.name
        );
        let code = self
            .elf
            .bytes_at(function.address, usize::try_from(function.size)?)
            .with_context(|| format!("the code of `{}` lies outside the image", function.name))?;
        let mut instructions = Vec::new();
        let mut offset = 0_usize;
        while let Some(rest) = code.get(offset..) {
            let address = function.address + u32::try_from(offset)?;
            if let Some(code_start) = self.literals_end(address, function) {
                offset = usize::try_from(code_start - function.address)?;
                continue;
            }
            let Some(instruction) = Instruction::read(rest) else {
                break;
            };
            offset += instruction.width();
            instructions.push((address, instruction));
        }
        Ok(instructions)
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
        let mut moves: BTreeMap<usize, (Vec<Half>, Vec<Half>)> = BTreeMap::new();
        let mut calls = 0_usize;
        let mut call_sites = HashSet::new();
        for relocation in self.elf.relocations()? {
            let at = relocation.offset;
            if relocation.kind == RelocationKind::Branch {
                call_sites.insert(at);
            }
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
                RelocationKind::MoveLow | RelocationKind::MoveHigh => {
                    let (register, value) = thumb::move_immediate(instruction()?);
                    let half = Half {
                        at,
                        register,
                        value,
                        symbol: relocation.symbol,
                    };
                    let (lows, highs) = moves.entry(source).or_default();
                    if relocation.kind == RelocationKind::MoveLow {
                        lows.push(half);
                    } else {
                        highs.push(half);
                    }
                    continue;
                }
            };
            let call = matches!(
                relocation.kind,
                RelocationKind::Branch | RelocationKind::ConditionalBranch
            );
            let referred = self.refer(source, at, target, call);
            calls += usize::from(call && referred);
        }
        for (source, (lows, highs)) in moves {
            let Some(function) = self.symbol(source).filter(|_| self.is_function(source)) else {
                bail!(
                    "halves of addresses are built outside a function, in `{}`",
                    self.name(source)
                );
            };
            let function = function.clone();
            let code = self.instructions(&function)?;
            for (at, target) in halves::join(&code, &lows, &highs)
                .with_context(|| format!("in `{}`", function.name))?
            {
                self.refer(source, at, target, false);
            }
        }
        for references in &mut self.references {
            references.sort_by_key(|reference| reference.at);
        }
        require_relocated_calls(calls)?;
        self.require_relocated_branches_with_link(&call_sites)
    }

    fn refer(&mut self, source: usize, at: u32, target: u32, call: bool) -> bool {
        let code = target & !1;
        let node = match self.node_at(code) {
            Some(node) if self.is_function(node) => Some(node),
            _ => self.node_at(target),
        };
        let (Some(node), Some(references)) = (node, self.references.get_mut(source)) else {
            return false;
        };
        references.push(Reference {
            at,
            target,
            node,
            call,
        });
        true
    }

    fn require_relocated_branches_with_link(&self, call_sites: &HashSet<u32>) -> Result<()> {
        for function in self
            .nodes
            .iter()
            .filter(|node| node.kind == SymbolKind::Function)
        {
            for (address, instruction) in self.instructions(function)? {
                ensure!(
                    !instruction.is_branch_with_link() || call_sites.contains(&address),
                    "the call at {} in `{}` has no relocation; boards link with `--emit-relocs`",
                    Hex(address),
                    function.name
                );
            }
        }
        Ok(())
    }
}

fn require_relocated_calls(calls: usize) -> Result<()> {
    ensure!(
        calls > 0,
        "the image has no relocations of its calls; boards link with `--emit-relocs`"
    );
    Ok(())
}

fn jumps_within(function: &Symbol, code: Vec<(u32, Instruction)>) -> Vec<(u32, u32)> {
    code.into_iter()
        .filter_map(|(address, instruction)| {
            instruction
                .unconditional_branch_target(address)
                .filter(|&target| function.contains(target))
                .map(|target| (address, target))
        })
        .collect()
}

fn with_unnamed<'s>(
    mut nodes: Vec<Symbol>,
    input_sections: impl Iterator<Item = &'s InputSection>,
) -> Vec<Symbol> {
    nodes.sort_by(|left, right| {
        left.address
            .cmp(&right.address)
            .then(right.size.cmp(&left.size))
    });
    nodes.dedup_by_key(|node| node.address);
    let named: HashSet<u32> = nodes.iter().map(|node| node.address).collect();
    let unnamed: Vec<Symbol> = input_sections
        .filter(|section| !named.contains(&section.address))
        .map(|section| Symbol {
            name: section.name.clone(),
            address: section.address,
            size: section.size,
            kind: SymbolKind::Object,
        })
        .collect();
    nodes.extend(unnamed);
    nodes.sort_by_key(|node| node.address);
    nodes.dedup_by_key(|node| node.address);
    nodes
}

fn reachable(
    len: usize,
    roots: impl IntoIterator<Item = usize>,
    successors: impl Fn(usize) -> Vec<usize>,
) -> Vec<usize> {
    let mut seen = vec![false; len];
    let mut queue: VecDeque<usize> = roots.into_iter().collect();
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
    fn an_image_without_relocated_calls_is_rejected() {
        assert_eq!(
            require_relocated_calls(0).unwrap_err().to_string(),
            "the image has no relocations of its calls; boards link with `--emit-relocs`"
        );
        assert!(require_relocated_calls(1).is_ok());
    }

    #[test]
    fn every_node_the_roots_reach_is_visited_once() {
        let edges: [&[usize]; 6] = [&[1, 2], &[2, 0], &[3], &[], &[0], &[4]];
        let mut nodes = reachable(edges.len(), [0], |node| edges[node].to_vec());
        nodes.sort_unstable();
        assert_eq!(nodes, [0, 1, 2, 3]);
        let mut nodes = reachable(edges.len(), [3, 5], |node| edges[node].to_vec());
        nodes.sort_unstable();
        assert_eq!(nodes, [0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn an_input_section_without_a_symbol_at_its_start_becomes_a_node_of_its_own() {
        let symbol = |name: &str, address, size, kind| Symbol {
            name: name.to_owned(),
            address,
            size,
            kind,
        };
        let section = |name: &str, address, size| InputSection {
            name: name.to_owned(),
            address,
            size,
        };
        let symbols = vec![
            symbol("VTABLE", 0x110, 0x10, SymbolKind::Object),
            symbol(".rodata", 0x100, 0, SymbolKind::Object),
            symbol("poll", 0x40, 0x20, SymbolKind::Function),
        ];
        let sections = [
            section(".text.poll", 0x40, 0x24),
            section(".rodata.VTABLE", 0x110, 0x10),
            section(".rodata..Lanon.1", 0x120, 0x10),
            section(".rodata..Lanon.2", 0x130, 0x8),
        ];
        let nodes = with_unnamed(symbols, sections.iter());
        let nodes: Vec<(&str, u32, SymbolKind)> = nodes
            .iter()
            .map(|node| (node.name.as_str(), node.address, node.kind))
            .collect();
        assert_eq!(
            nodes,
            [
                ("poll", 0x40, SymbolKind::Function),
                (".rodata", 0x100, SymbolKind::Object),
                ("VTABLE", 0x110, SymbolKind::Object),
                (".rodata..Lanon.1", 0x120, SymbolKind::Object),
                (".rodata..Lanon.2", 0x130, SymbolKind::Object),
            ]
        );
    }

    #[test]
    fn only_unconditional_branches_to_the_inside_of_the_function_are_jumps() {
        let function = Symbol {
            name: "poll".to_owned(),
            address: 0x0806_F000,
            size: 0x100,
            kind: SymbolKind::Function,
        };
        let code = vec![
            (0x0806_F010, Instruction::Narrow(0xE002)),
            (0x0806_F020, Instruction::Wide([0xF047, 0xFC6D])),
            (0x0806_F030, Instruction::Wide([0xF040, 0x8666])),
            (0x0806_F040, Instruction::Narrow(0xD002)),
            (0x0806_F07A, Instruction::Wide([0xF7FF, 0xBA3E])),
            (0x0806_F0FE, Instruction::Narrow(0xE002)),
        ];
        assert_eq!(jumps_within(&function, code), [(0x0806_F010, 0x0806_F018)]);
    }
}
