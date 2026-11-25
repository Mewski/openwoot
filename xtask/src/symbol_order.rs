use std::collections::HashSet;

use crate::{call_graph::CallGraph, entry_points::EntryPoints};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Function<'a> {
    link_name: &'a str,
    size: u32,
}

pub(crate) fn symbol_order(graph: &CallGraph<'_>, entry_points: &EntryPoints) -> String {
    let functions = |nodes: Vec<usize>| -> Vec<Function<'_>> {
        nodes
            .into_iter()
            .filter(|&node| graph.is_function(node))
            .filter_map(|node| graph.symbol(node))
            .map(|symbol| Function {
                link_name: &symbol.link_name,
                size: symbol.size,
            })
            .collect()
    };
    let time_critical = entry_points
        .interrupts
        .iter()
        .chain(&entry_points.key_tasks)
        .copied();
    let mut first = functions(graph.reachable(time_critical));
    first.extend(functions(entry_points.thread_executors.clone()));
    let tasks = entry_points
        .thread_tasks
        .iter()
        .map(|&task| functions(graph.reachable([task])))
        .collect();
    let mut listed = String::new();
    for link_name in order(first, tasks) {
        listed.push_str(link_name);
        listed.push('\n');
    }
    listed
}

fn order<'a>(mut first: Vec<Function<'a>>, tasks: Vec<Vec<Function<'a>>>) -> Vec<&'a str> {
    let mut placed = HashSet::new();
    let mut names = Vec::new();
    place(&mut first, &mut placed, &mut names);
    let mut tasks: Vec<(u32, Vec<Function<'a>>)> = tasks
        .into_iter()
        .map(|functions| (new_code(&functions, &placed), functions))
        .collect();
    tasks.sort_by_key(|(size, functions)| {
        (
            *size,
            functions.iter().map(|function| function.link_name).min(),
        )
    });
    for (_, mut functions) in tasks {
        place(&mut functions, &mut placed, &mut names);
    }
    names
}

fn place<'a>(
    functions: &mut [Function<'a>],
    placed: &mut HashSet<&'a str>,
    names: &mut Vec<&'a str>,
) {
    functions.sort_unstable_by_key(|function| function.link_name);
    for function in functions.iter() {
        if placed.insert(function.link_name) {
            names.push(function.link_name);
        }
    }
}

fn new_code(functions: &[Function<'_>], placed: &HashSet<&str>) -> u32 {
    let mut counted = HashSet::new();
    functions
        .iter()
        .filter(|function| !placed.contains(function.link_name))
        .filter(|function| counted.insert(function.link_name))
        .map(|function| function.size)
        .fold(0, u32::saturating_add)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn functions<'a>(named: &[(&'a str, u32)]) -> Vec<Function<'a>> {
        named
            .iter()
            .map(|&(link_name, size)| Function { link_name, size })
            .collect()
    }

    #[test]
    fn time_critical_code_comes_first_sorted_by_name() {
        assert_eq!(
            order(
                functions(&[("scan", 40), ("memcpy", 8), ("scan", 40)]),
                vec![functions(&[("render", 900), ("memcpy", 8)])]
            ),
            ["memcpy", "scan", "render"]
        );
    }

    #[test]
    fn thread_tasks_follow_with_the_least_new_code_first() {
        let first = functions(&[("poll", 30)]);
        let configuration = functions(&[("poll", 30), ("serve", 5000), ("encode", 2000)]);
        let reports = functions(&[("poll", 30), ("forward", 200)]);
        let lighting = functions(&[("render", 900), ("forward", 200)]);
        assert_eq!(
            order(first, vec![configuration, lighting, reports]),
            ["poll", "forward", "render", "encode", "serve"]
        );
    }

    #[test]
    fn tasks_with_as_much_new_code_keep_a_fixed_order() {
        let order_of = |tasks: Vec<Vec<Function<'static>>>| order(Vec::new(), tasks);
        let device = functions(&[("control", 100)]);
        let reports = functions(&[("forward", 100)]);
        assert_eq!(
            order_of(vec![device.clone(), reports.clone()]),
            order_of(vec![reports, device])
        );
    }
}
