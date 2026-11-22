use std::{fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use cargo_metadata::Package;

use crate::workspace::{self, Workspace};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Firmware,
    Protocol,
    Drivers,
    Image,
    Pac,
    ChipHal,
    Board,
    Cli,
    Xtask,
}

impl Kind {
    pub(super) const fn is_shared(self) -> bool {
        matches!(
            self,
            Self::Firmware | Self::Protocol | Self::Drivers | Self::Image
        )
    }

    const fn allowed_dependencies(self) -> &'static [Self] {
        match self {
            Self::Protocol | Self::Drivers | Self::Image | Self::Pac => &[],
            Self::Firmware => &[Self::Protocol, Self::Drivers, Self::Image],
            Self::ChipHal => &[Self::Firmware, Self::Pac, Self::Image],
            Self::Board => &[
                Self::Firmware,
                Self::Protocol,
                Self::Drivers,
                Self::Image,
                Self::ChipHal,
                Self::Pac,
            ],
            Self::Cli => &[Self::Protocol, Self::Image],
            Self::Xtask => &[Self::Image],
        }
    }
}

pub(super) struct Member<'a> {
    pub(super) package: &'a Package,
    pub(super) kind: Kind,
    pub(super) directory: PathBuf,
}

pub(super) fn classify(workspace: &Workspace) -> Result<Vec<Member<'_>>> {
    workspace
        .members()
        .map(|package| {
            let kind = match package.name.as_str() {
                "openwoot" => Kind::Firmware,
                "openwoot-protocol" => Kind::Protocol,
                "openwoot-drivers" => Kind::Drivers,
                "openwoot-image" => Kind::Image,
                "openwoot-cli" => Kind::Cli,
                "xtask" => Kind::Xtask,
                _ if workspace.is_pac(package) => Kind::Pac,
                _ if workspace.is_board(package) => Kind::Board,
                _ if workspace::chip_target(package).is_some() => Kind::ChipHal,
                name => bail!("package `{name}` belongs to no known kind of crate"),
            };
            Ok(Member {
                package,
                kind,
                directory: Workspace::package_directory(package),
            })
        })
        .collect()
}

pub(super) fn dependency_direction(members: &[Member<'_>], problems: &mut Vec<String>) {
    for member in members {
        for dependency in &member.package.dependencies {
            let Some(target) = members
                .iter()
                .find(|candidate| candidate.package.name.as_str() == dependency.name)
            else {
                continue;
            };
            if !member.kind.allowed_dependencies().contains(&target.kind) {
                problems.push(format!(
                    "`{}` ({:?}) must not depend on `{}` ({:?})",
                    member.package.name, member.kind, target.package.name, target.kind
                ));
            }
        }
    }
}

pub(super) fn lints_tables(members: &[Member<'_>], problems: &mut Vec<String>) -> Result<()> {
    for member in members.iter().filter(|member| member.kind != Kind::Pac) {
        let manifest_path = &member.package.manifest_path;
        let manifest = fs::read_to_string(manifest_path)
            .with_context(|| format!("could not read `{manifest_path}`"))?;
        if !manifest.contains("[lints]\nworkspace = true") {
            problems.push(format!(
                "`{}`: `Cargo.toml` needs `[lints] workspace = true`",
                member.package.name
            ));
        }
    }
    Ok(())
}
