use std::{collections::BTreeMap, fs};

use anyhow::{Context, Result};
use cargo_metadata::TargetKind;
use toml::{Table, Value};

use crate::{
    call_graph::CallGraph,
    elf::CompileUnits,
    entry_points::{EntryPoints, Executor},
    workspace::Workspace,
};

const SIZE_LEVELS: [&str; 2] = ["s", "z"];
const DEFAULT_LEVEL: &str = "3";
const EVERY_DEPENDENCY: &str = "*";

pub(super) struct OptLevels {
    crates: BTreeMap<String, String>,
    dependencies: String,
}

impl OptLevels {
    pub(super) fn read(workspace: &Workspace) -> Result<Self> {
        let path = workspace.root.join("Cargo.toml");
        let manifest = fs::read_to_string(&path)
            .with_context(|| format!("could not read `{}`", path.display()))?;
        let manifest: Table = manifest
            .parse()
            .with_context(|| format!("could not parse `{}`", path.display()))?;
        let release = manifest
            .get("profile")
            .and_then(|profile| profile.get("release"));
        let default = release
            .and_then(|release| release.get("opt-level"))
            .map_or_else(|| DEFAULT_LEVEL.to_owned(), level);
        let packages = release
            .and_then(|release| release.get("package"))
            .and_then(Value::as_table);
        let level_of = |package: &str| {
            packages
                .and_then(|packages| packages.get(package))
                .and_then(|settings| settings.get("opt-level"))
                .map(level)
        };
        let mut crates = BTreeMap::new();
        for package in workspace.members() {
            let package_level = level_of(&package.name).unwrap_or_else(|| default.clone());
            for target in &package.targets {
                if target
                    .kind
                    .iter()
                    .any(|kind| matches!(kind, TargetKind::Lib | TargetKind::Bin))
                {
                    crates.insert(target.name.replace('-', "_"), package_level.clone());
                }
            }
        }
        Ok(Self {
            crates,
            dependencies: level_of(EVERY_DEPENDENCY).unwrap_or(default),
        })
    }

    fn of(&self, crate_name: &str) -> &str {
        self.crates.get(crate_name).unwrap_or(&self.dependencies)
    }
}

pub(super) fn size_compiled(
    graph: &CallGraph<'_>,
    entry_points: &EntryPoints,
    units: &CompileUnits,
    levels: &OptLevels,
) -> Vec<String> {
    let mut reached_from = BTreeMap::new();
    let hot_tasks = Executor::ALL
        .into_iter()
        .filter(|executor| executor.runs_for_speed())
        .flat_map(|executor| entry_points.tasks_of(executor));
    for &root in entry_points.interrupts.iter().chain(hot_tasks) {
        for node in graph.reachable([root]) {
            reached_from.entry(node).or_insert(root);
        }
    }
    reached_from
        .into_iter()
        .filter(|&(node, _)| graph.is_function(node))
        .filter_map(|(node, root)| {
            let function = graph.symbol(node)?;
            let crate_name = units.crate_at(function.address)?;
            let level = levels.of(crate_name);
            SIZE_LEVELS.contains(&level).then(|| {
                format!(
                    "`{}` runs `{}`, which `{crate_name}` compiles for size (opt-level `{level}`)",
                    graph.name(root),
                    function.name
                )
            })
        })
        .collect()
}

fn level(value: &Value) -> String {
    value.as_str().map_or_else(
        || {
            value
                .as_integer()
                .map_or_else(String::new, |level| level.to_string())
        },
        str::to_owned,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_read_as_written_in_the_manifest() {
        assert_eq!(level(&Value::Integer(3)), "3");
        assert_eq!(level(&Value::String("z".to_owned())), "z");
    }

    #[test]
    fn a_crate_without_its_own_level_takes_the_level_of_every_dependency() {
        let levels = OptLevels {
            crates: BTreeMap::from([("wooting_80he".to_owned(), "z".to_owned())]),
            dependencies: "3".to_owned(),
        };
        assert_eq!(levels.of("wooting_80he"), "z");
        assert_eq!(levels.of("embassy_executor"), "3");
    }
}
