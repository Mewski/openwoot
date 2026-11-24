use std::{collections::BTreeSet, fmt, fs, path::PathBuf};

use anyhow::{Context, Result, bail};
use cargo_metadata::Package;
use toml::{Table, Value};

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
    Host,
    Cli,
    Gui,
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
            Self::Host => &[Self::Protocol, Self::Image],
            Self::Cli | Self::Gui => &[Self::Protocol, Self::Image, Self::Host],
            Self::Xtask => &[Self::Image],
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Firmware => "firmware",
            Self::Protocol => "protocol",
            Self::Drivers => "drivers",
            Self::Image => "image",
            Self::Pac => "PAC",
            Self::ChipHal => "chip HAL",
            Self::Board => "board",
            Self::Host => "host",
            Self::Cli => "CLI",
            Self::Gui => "GUI",
            Self::Xtask => "xtask",
        })
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
                "openwoot-host" => Kind::Host,
                "openwoot-cli" => Kind::Cli,
                "openwoot-gui" => Kind::Gui,
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
                    "`{}` ({}) must not depend on `{}` ({})",
                    member.package.name, member.kind, target.package.name, target.kind
                ));
            }
        }
    }
}

pub(super) fn peripheral_access_crates(members: &[Member<'_>], problems: &mut Vec<String>) {
    for member in members {
        let pacs = dependencies_of_kind(member, members, Kind::Pac);
        match member.kind {
            Kind::ChipHal if pacs.len() != 1 => problems.push(format!(
                "`{}` ({}) must depend on exactly one PAC, not {}",
                member.package.name,
                member.kind,
                pacs.len()
            )),
            Kind::Board => {
                let own: BTreeSet<&str> = dependencies_of_kind(member, members, Kind::ChipHal)
                    .into_iter()
                    .filter_map(|hal| {
                        members
                            .iter()
                            .find(|candidate| candidate.package.name.as_str() == hal)
                    })
                    .flat_map(|hal| dependencies_of_kind(hal, members, Kind::Pac))
                    .collect();
                for pac in pacs.difference(&own) {
                    problems.push(format!(
                        "`{}` ({}) must not depend on `{pac}`, the PAC of another chip",
                        member.package.name, member.kind
                    ));
                }
            }
            _ => {}
        }
    }
}

fn dependencies_of_kind<'m>(
    member: &Member<'_>,
    members: &'m [Member<'_>],
    kind: Kind,
) -> BTreeSet<&'m str> {
    member
        .package
        .dependencies
        .iter()
        .filter_map(|dependency| {
            members
                .iter()
                .find(|candidate| candidate.package.name.as_str() == dependency.name)
        })
        .filter(|target| target.kind == kind)
        .map(|target| target.package.name.as_str())
        .collect()
}

pub(super) fn lints_tables(members: &[Member<'_>], problems: &mut Vec<String>) -> Result<()> {
    for member in members.iter().filter(|member| member.kind != Kind::Pac) {
        let manifest_path = &member.package.manifest_path;
        let manifest = fs::read_to_string(manifest_path)
            .with_context(|| format!("could not read `{manifest_path}`"))?;
        let inherits = inherits_workspace_lints(&manifest)
            .with_context(|| format!("could not parse `{manifest_path}`"))?;
        if !inherits {
            problems.push(format!(
                "`{}`: `Cargo.toml` needs `[lints] workspace = true`",
                member.package.name
            ));
        }
    }
    Ok(())
}

fn inherits_workspace_lints(manifest: &str) -> Result<bool> {
    let manifest: Table = manifest.parse()?;
    let workspace = manifest
        .get("lints")
        .and_then(|lints| lints.get("workspace"))
        .and_then(Value::as_bool);
    Ok(workspace == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lints_table_inherits_regardless_of_spelling() {
        for manifest in [
            "[lints]\nworkspace = true\n",
            "[lints]\r\nworkspace=true\r\n",
            "[lints]\n\n  workspace   =   true  # inherited\n",
            "lints = { workspace = true }\n",
            "lints.workspace = true\n",
        ] {
            assert!(inherits_workspace_lints(manifest).unwrap(), "{manifest:?}");
        }
    }

    #[test]
    fn missing_or_disabled_lints_table_is_reported() {
        for manifest in [
            "[package]\nname = \"crate\"\n",
            "[lints]\nworkspace = false\n",
            "[lints.rust]\nworkspace = true\n",
            "# [lints]\n# workspace = true\n",
        ] {
            assert!(!inherits_workspace_lints(manifest).unwrap(), "{manifest:?}");
        }
    }

    #[test]
    fn malformed_manifest_is_an_error() {
        assert!(inherits_workspace_lints("[lints\nworkspace = true\n").is_err());
    }

    #[test]
    fn only_the_documented_dependency_directions_are_allowed() {
        assert!(Kind::Board.allowed_dependencies().contains(&Kind::ChipHal));
        assert!(Kind::ChipHal.allowed_dependencies().contains(&Kind::Pac));
        assert!(
            Kind::Firmware
                .allowed_dependencies()
                .contains(&Kind::Drivers)
        );
        assert!(
            !Kind::Firmware
                .allowed_dependencies()
                .contains(&Kind::ChipHal)
        );
        assert!(!Kind::Board.allowed_dependencies().contains(&Kind::Cli));
        assert!(!Kind::Cli.allowed_dependencies().contains(&Kind::Firmware));
        assert!(!Kind::Board.allowed_dependencies().contains(&Kind::Host));
        assert!(!Kind::Host.allowed_dependencies().contains(&Kind::Cli));
        assert!(!Kind::Host.allowed_dependencies().contains(&Kind::Firmware));
        assert!(!Kind::Gui.allowed_dependencies().contains(&Kind::Cli));
        assert_eq!(
            Kind::Host.allowed_dependencies(),
            [Kind::Protocol, Kind::Image]
        );
        assert_eq!(
            Kind::Cli.allowed_dependencies(),
            [Kind::Protocol, Kind::Image, Kind::Host]
        );
        assert_eq!(
            Kind::Gui.allowed_dependencies(),
            [Kind::Protocol, Kind::Image, Kind::Host]
        );
        assert_eq!(Kind::Drivers.allowed_dependencies(), []);
        assert_eq!(Kind::Xtask.allowed_dependencies(), [Kind::Image]);
    }

    #[test]
    fn kinds_read_as_words_in_messages() {
        assert_eq!(Kind::ChipHal.to_string(), "chip HAL");
        assert_eq!(Kind::Board.to_string(), "board");
        assert_eq!(Kind::Pac.to_string(), "PAC");
        assert_eq!(Kind::Host.to_string(), "host");
        assert_eq!(Kind::Gui.to_string(), "GUI");
    }
}
