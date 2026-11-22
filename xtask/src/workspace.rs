use std::{
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail, ensure};
use cargo_metadata::{Metadata, MetadataCommand, Package, TargetKind};

#[derive(Debug, Clone)]
pub(crate) struct Board {
    pub(crate) name: String,
    pub(crate) binary: String,
    pub(crate) chip_hal: String,
    pub(crate) target: String,
    pub(crate) directory: PathBuf,
}

pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    pub(crate) metadata: Metadata,
    boards: Vec<Board>,
}

impl Workspace {
    pub(crate) fn load(root: &Path) -> Result<Self> {
        let metadata = MetadataCommand::new()
            .current_dir(root)
            .no_deps()
            .exec()
            .context("cargo metadata failed")?;
        let mut workspace = Self {
            root: root.to_path_buf(),
            metadata,
            boards: Vec::new(),
        };
        let mut boards = workspace
            .members()
            .filter(|package| workspace.is_board(package))
            .map(|package| workspace.describe_board(package))
            .collect::<Result<Vec<_>>>()?;
        boards.sort_by(|left, right| left.name.cmp(&right.name));
        workspace.boards = boards;
        Ok(workspace)
    }

    pub(crate) fn members(&self) -> impl Iterator<Item = &Package> {
        self.metadata.workspace_packages().into_iter()
    }

    pub(crate) fn boards(&self) -> &[Board] {
        &self.boards
    }

    pub(crate) fn board(&self, name: &str) -> Result<&Board> {
        self.boards
            .iter()
            .find(|board| board.name == name)
            .with_context(|| {
                let known: Vec<&str> = self
                    .boards
                    .iter()
                    .map(|board| board.name.as_str())
                    .collect();
                format!("unknown board `{name}`; boards: {}", known.join(", "))
            })
    }

    pub(crate) fn package_directory(package: &Package) -> PathBuf {
        package
            .manifest_path
            .parent()
            .map_or_else(PathBuf::new, PathBuf::from)
    }

    pub(crate) fn is_board(&self, package: &Package) -> bool {
        Self::package_directory(package).parent() == Some(self.root.join("boards").as_path())
    }

    pub(crate) fn is_pac(&self, package: &Package) -> bool {
        let directory = Self::package_directory(package);
        directory.parent() == Some(self.root.join("crates").as_path())
            && package.name.ends_with("-pac")
    }

    pub(crate) fn target_directory(&self) -> &Path {
        self.metadata.target_directory.as_std_path()
    }

    fn describe_board(&self, package: &Package) -> Result<Board> {
        let directory = Self::package_directory(package);
        let directory_name = directory
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        ensure!(
            directory_name == package.name.as_str(),
            "board package `{}` lives in boards/{directory_name}",
            package.name
        );
        let binaries: Vec<&str> = package
            .targets
            .iter()
            .filter(|target| target.kind.contains(&TargetKind::Bin))
            .map(|target| target.name.as_str())
            .collect();
        let [binary] = binaries.as_slice() else {
            bail!("board `{}` must have exactly one binary", package.name);
        };
        let chip_hals: Vec<(&str, &str)> = package
            .dependencies
            .iter()
            .filter_map(|dependency| {
                self.members()
                    .find(|member| member.name.as_str() == dependency.name)
                    .and_then(|member| Some((member.name.as_str(), chip_target(member)?)))
            })
            .collect();
        let [(chip_hal, target)] = chip_hals.as_slice() else {
            bail!(
                "board `{}` must depend on exactly one chip HAL",
                package.name
            );
        };
        Ok(Board {
            name: package.name.to_string(),
            binary: (*binary).to_owned(),
            chip_hal: (*chip_hal).to_owned(),
            target: (*target).to_owned(),
            directory,
        })
    }
}

pub(crate) fn chip_target(package: &Package) -> Option<&str> {
    package.metadata.get("openwoot")?.get("target")?.as_str()
}

pub(crate) fn cargo(root: &Path) -> Command {
    let mut command = Command::new("cargo");
    command.current_dir(root);
    command
}

pub(crate) fn execute(command: &mut Command) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("could not start {command:?}"))?;
    ensure!(status.success(), "{command:?} failed with {status}");
    Ok(())
}
