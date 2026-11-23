const UNVISITED: usize = usize::MAX;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Components {
    of: Vec<usize>,
    members: Vec<Vec<usize>>,
    successors: Vec<Vec<usize>>,
    cyclic: Vec<bool>,
}

impl Components {
    pub(super) fn new(len: usize, successors_of: impl Fn(usize) -> Vec<usize>) -> Self {
        let edges: Vec<Vec<usize>> = (0..len).map(successors_of).collect();
        let mut search = Search {
            edges: &edges,
            order: vec![UNVISITED; len],
            lowest: vec![0; len],
            on_stack: vec![false; len],
            stack: Vec::new(),
            visited: 0,
            of: vec![0; len],
            members: Vec::new(),
        };
        for start in 0..len {
            if at(&search.order, start) == UNVISITED {
                search.visit(start);
            }
        }
        let Search { of, members, .. } = search;
        let mut successors = vec![Vec::new(); members.len()];
        let mut cyclic = vec![false; members.len()];
        for (node, targets) in edges.iter().enumerate() {
            let from = at(&of, node);
            for &target in targets {
                let to = at(&of, target);
                if to == from {
                    set(&mut cyclic, from, true);
                } else if let Some(list) = successors.get_mut(from) {
                    list.push(to);
                }
            }
        }
        for list in &mut successors {
            list.sort_unstable();
            list.dedup();
        }
        Self {
            of,
            members,
            successors,
            cyclic,
        }
    }

    pub(super) fn of(&self, node: usize) -> usize {
        at(&self.of, node)
    }

    pub(super) fn members(&self, component: usize) -> &[usize] {
        self.members.get(component).map_or(&[], Vec::as_slice)
    }

    pub(super) fn successors(&self, component: usize) -> &[usize] {
        self.successors.get(component).map_or(&[], Vec::as_slice)
    }

    pub(super) fn cycles(&self) -> impl Iterator<Item = &[usize]> {
        self.members
            .iter()
            .zip(&self.cyclic)
            .filter(|(_, cyclic)| **cyclic)
            .map(|(members, _)| members.as_slice())
    }
}

struct Search<'e> {
    edges: &'e [Vec<usize>],
    order: Vec<usize>,
    lowest: Vec<usize>,
    on_stack: Vec<bool>,
    stack: Vec<usize>,
    visited: usize,
    of: Vec<usize>,
    members: Vec<Vec<usize>>,
}

impl Search<'_> {
    fn visit(&mut self, start: usize) {
        let mut work = vec![(start, 0_usize)];
        while let Some((node, position)) = work.pop() {
            if position == 0 {
                self.enter(node);
            }
            let edges = self.edges;
            if let Some(&target) = edges.get(node).and_then(|targets| targets.get(position)) {
                work.push((node, position + 1));
                if at(&self.order, target) == UNVISITED {
                    work.push((target, 0));
                } else if at(&self.on_stack, target) {
                    lower(&mut self.lowest, node, at(&self.order, target));
                }
                continue;
            }
            if at(&self.lowest, node) == at(&self.order, node) {
                self.close(node);
            }
            if let Some(&(parent, _)) = work.last() {
                let lowest = at(&self.lowest, node);
                lower(&mut self.lowest, parent, lowest);
            }
        }
    }

    fn enter(&mut self, node: usize) {
        set(&mut self.order, node, self.visited);
        set(&mut self.lowest, node, self.visited);
        set(&mut self.on_stack, node, true);
        self.stack.push(node);
        self.visited += 1;
    }

    fn close(&mut self, root: usize) {
        let component = self.members.len();
        let mut members = Vec::new();
        while let Some(member) = self.stack.pop() {
            set(&mut self.on_stack, member, false);
            set(&mut self.of, member, component);
            members.push(member);
            if member == root {
                break;
            }
        }
        members.sort_unstable();
        self.members.push(members);
    }
}

fn at<Value: Copy + Default>(values: &[Value], index: usize) -> Value {
    values.get(index).copied().unwrap_or_default()
}

fn set<Value>(values: &mut [Value], index: usize, value: Value) {
    if let Some(slot) = values.get_mut(index) {
        *slot = value;
    }
}

fn lower(values: &mut [usize], index: usize, value: usize) {
    if let Some(slot) = values.get_mut(index) {
        *slot = (*slot).min(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn components(edges: &[&[usize]]) -> Components {
        Components::new(edges.len(), |node| edges[node].to_vec())
    }

    #[test]
    fn nodes_on_a_cycle_share_one_component() {
        let found = components(&[&[1], &[2], &[0, 3], &[]]);
        assert_eq!(found.of(0), found.of(1));
        assert_eq!(found.of(1), found.of(2));
        assert_ne!(found.of(2), found.of(3));
        assert_eq!(found.members(found.of(0)), [0, 1, 2]);
        assert_eq!(found.successors(found.of(0)), [found.of(3)]);
        assert_eq!(found.cycles().collect::<Vec<_>>(), [&[0, 1, 2][..]]);
    }

    #[test]
    fn a_node_that_reaches_itself_directly_is_a_cycle() {
        let found = components(&[&[0, 1], &[]]);
        assert_eq!(found.cycles().collect::<Vec<_>>(), [&[0][..]]);
        assert_eq!(found.successors(found.of(0)), [found.of(1)]);
    }

    #[test]
    fn a_graph_without_cycles_keeps_every_node_apart() {
        let found = components(&[&[1, 2], &[2], &[], &[2, 2]]);
        assert_eq!(found.cycles().count(), 0);
        for node in 0..4 {
            assert_eq!(found.members(found.of(node)), [node]);
        }
        assert_eq!(found.successors(found.of(3)), [found.of(2)]);
        assert_eq!(found.successors(found.of(0)).len(), 2);
    }

    #[test]
    fn two_cycles_joined_by_one_edge_stay_two_components() {
        let found = components(&[&[1], &[0, 2], &[3], &[2]]);
        assert_eq!(found.members(found.of(0)), [0, 1]);
        assert_eq!(found.members(found.of(2)), [2, 3]);
        assert_eq!(found.successors(found.of(0)), [found.of(2)]);
        assert_eq!(found.successors(found.of(2)), [] as [usize; 0]);
        assert_eq!(found.cycles().count(), 2);
    }
}
