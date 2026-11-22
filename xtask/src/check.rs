use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use cargo_metadata::Package;
use openwoot_image::{Descriptor, Hex, Image, MemoryLayout};

use crate::{
    firmware::{self, Elf},
    rust_source,
    workspace::{self, Board, Workspace},
};

const FORBIDDEN_EXTENSIONS: [&str; 19] = [
    "asm", "bash", "bat", "c", "cc", "cmd", "cpp", "cxx", "fish", "h", "hh", "hpp", "hxx", "mk",
    "ps1", "py", "s", "sh", "zsh",
];
const FORBIDDEN_FILE_NAMES: [&str; 4] = ["CMakeLists.txt", "GNUmakefile", "Makefile", "makefile"];
const VAGUE_MODULE_NAMES: [&str; 5] = ["common", "helpers", "misc", "types", "utils"];
const SAFETY_NOTE: &str = "// SAFETY:";
const UNSAFE_KEYWORD: &str = "unsafe";
const VECTOR_SIZE: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
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

    const fn is_shared(self) -> bool {
        matches!(
            self,
            Self::Firmware | Self::Protocol | Self::Drivers | Self::Image
        )
    }
}

struct Member<'a> {
    package: &'a Package,
    kind: Kind,
    directory: PathBuf,
}

pub(crate) fn run(root: &Path) -> Result<()> {
    let workspace = Workspace::load(root)?;
    let members = classify(&workspace)?;
    let files = tree_files(root)?;

    let mut problems = Vec::new();
    forbidden_files(root, &files, &mut problems);
    dependency_direction(&members, &mut problems);
    lints_tables(&members, &mut problems)?;
    file_set_symmetry(&members, Kind::ChipHal, &files, &mut problems);
    file_set_symmetry(&members, Kind::Board, &files, &mut problems);
    shared_code_names(&workspace, &members, &files, &mut problems)?;
    rust_rules(root, &members, &files, &mut problems)?;
    if !problems.is_empty() {
        for problem in &problems {
            eprintln!("{problem}");
        }
        bail!("{} problems in the tree", problems.len());
    }

    formatting(root)?;
    clippy(&workspace, &members)?;
    for board in workspace.boards() {
        board_image(&workspace, board)
            .with_context(|| format!("could not check the image of `{}`", board.name))?;
    }
    crate::show("check", "passed", 0);
    Ok(())
}

fn classify(workspace: &Workspace) -> Result<Vec<Member<'_>>> {
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

fn tree_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let skipped = name.starts_with('.') && name != ".cargo" && name != ".github"
                || name == "target"
                || path == root.join("build");
            if skipped {
                continue;
            }
            if path.is_dir() {
                directories.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn forbidden_files(root: &Path, files: &[PathBuf], problems: &mut Vec<String>) {
    for file in files {
        let name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let extension = file
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default();
        if FORBIDDEN_FILE_NAMES.contains(&name)
            || FORBIDDEN_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        {
            problems.push(format!(
                "{}: not a Rust tree file type",
                relative(root, file)
            ));
        }
    }
}

fn dependency_direction(members: &[Member<'_>], problems: &mut Vec<String>) {
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

fn lints_tables(members: &[Member<'_>], problems: &mut Vec<String>) -> Result<()> {
    for member in members.iter().filter(|member| member.kind != Kind::Pac) {
        let manifest = fs::read_to_string(&member.package.manifest_path)?;
        if !manifest.contains("[lints]\nworkspace = true") {
            problems.push(format!(
                "`{}`: `Cargo.toml` needs `[lints] workspace = true`",
                member.package.name
            ));
        }
    }
    Ok(())
}

fn file_set_symmetry(
    members: &[Member<'_>],
    kind: Kind,
    files: &[PathBuf],
    problems: &mut Vec<String>,
) {
    let sets: Vec<(&str, BTreeSet<PathBuf>)> = members
        .iter()
        .filter(|member| member.kind == kind)
        .map(|member| {
            let set = files
                .iter()
                .filter_map(|file| file.strip_prefix(&member.directory).ok())
                .map(Path::to_path_buf)
                .collect();
            (member.package.name.as_str(), set)
        })
        .collect();
    let Some((first_name, first_set)) = sets.first() else {
        return;
    };
    for (name, set) in sets.iter().skip(1) {
        for file in first_set.symmetric_difference(set) {
            problems.push(format!(
                "{kind:?} crates differ: `{}` is in only one of `{first_name}` and `{name}`",
                file.display()
            ));
        }
    }
}

fn shared_code_names(
    workspace: &Workspace,
    members: &[Member<'_>],
    files: &[PathBuf],
    problems: &mut Vec<String>,
) -> Result<()> {
    let mut names = BTreeSet::new();
    for member in members {
        let name = member.package.name.as_str();
        match member.kind {
            Kind::Board => {
                names.insert(name.to_owned());
                names.insert(name.replace('-', "_"));
            }
            Kind::ChipHal => {
                names.insert(name.trim_start_matches("openwoot-").to_owned());
            }
            Kind::Pac => {
                names.insert(name.trim_end_matches("-pac").to_owned());
            }
            _ => {}
        }
    }
    for member in members.iter().filter(|member| member.kind.is_shared()) {
        for file in files
            .iter()
            .filter(|file| file.starts_with(&member.directory))
        {
            let text = fs::read_to_string(file)?.to_lowercase();
            for name in names.iter().filter(|name| text.contains(name.as_str())) {
                problems.push(format!(
                    "{}: shared code names the board or chip `{name}`",
                    relative(&workspace.root, file)
                ));
            }
        }
    }
    Ok(())
}

fn rust_rules(
    root: &Path,
    members: &[Member<'_>],
    files: &[PathBuf],
    problems: &mut Vec<String>,
) -> Result<()> {
    for file in files
        .iter()
        .filter(|file| file.extension().is_some_and(|extension| extension == "rs"))
    {
        let owner = members
            .iter()
            .filter(|member| file.starts_with(&member.directory))
            .max_by_key(|member| member.directory.components().count());
        let kind = owner.map(|member| member.kind);
        if kind == Some(Kind::Pac) {
            continue;
        }
        let shown = relative(root, file);
        if is_vague_module(file) {
            problems.push(format!(
                "{shown}: module name says nothing about its content"
            ));
        }
        let scanned = rust_source::scan(&fs::read_to_string(file)?);
        for comment in &scanned.comments {
            if !is_in_safety_note(&scanned, comment.line) {
                problems.push(format!(
                    "{shown}:{}: comment outside a `// SAFETY:` note",
                    comment.line
                ));
            }
        }
        for line in scanned.keyword_lines(UNSAFE_KEYWORD) {
            if kind != Some(Kind::ChipHal) {
                problems.push(format!("{shown}:{line}: `unsafe` outside a chip HAL"));
            } else if !has_safety_note(&scanned, line) {
                problems.push(format!(
                    "{shown}:{line}: `unsafe` without a `// SAFETY:` note"
                ));
            }
        }
    }
    Ok(())
}

fn is_vague_module(file: &Path) -> bool {
    let stem = file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let module = if stem == "mod" {
        file.parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    } else {
        stem
    };
    VAGUE_MODULE_NAMES.contains(&module)
}

fn is_comment_only(scanned: &rust_source::Scanned, line: usize) -> bool {
    scanned.code_on(line).trim().is_empty()
        && scanned
            .comment_on(line)
            .is_some_and(|comment| comment.text.starts_with("//"))
}

fn is_in_safety_note(scanned: &rust_source::Scanned, line: usize) -> bool {
    let mut start = line;
    while start > 1 && is_comment_only(scanned, start) && is_comment_only(scanned, start - 1) {
        start -= 1;
    }
    scanned
        .comment_on(start)
        .is_some_and(|comment| comment.text.starts_with(SAFETY_NOTE))
}

fn has_safety_note(scanned: &rust_source::Scanned, line: usize) -> bool {
    if scanned
        .comment_on(line)
        .is_some_and(|comment| comment.text.starts_with(SAFETY_NOTE))
    {
        return true;
    }
    let mut above = line.saturating_sub(1);
    while above > 0 && scanned.code_on(above).trim_start().starts_with("#[") {
        above -= 1;
    }
    above > 0 && is_comment_only(scanned, above) && is_in_safety_note(scanned, above)
}

fn formatting(root: &Path) -> Result<()> {
    workspace::execute(
        workspace::cargo(root)
            .env_remove("RUSTUP_TOOLCHAIN")
            .args(["+nightly", "fmt", "--all", "--", "--check"]),
    )?;
    workspace::execute(
        std::process::Command::new("taplo")
            .current_dir(root)
            .args(["fmt", "--check"]),
    )?;
    workspace::execute(workspace::cargo(root).args(["sort", "--workspace", "--check"]))
}

fn clippy(workspace: &Workspace, members: &[Member<'_>]) -> Result<()> {
    let mut host = workspace::cargo(&workspace.root);
    host.args(["clippy", "--workspace", "--all-targets"]);
    for member in members
        .iter()
        .filter(|member| matches!(member.kind, Kind::Pac | Kind::Board))
    {
        host.args(["--exclude", member.package.name.as_str()]);
    }
    workspace::execute(host.args(["--", "-D", "warnings"]))?;

    let mut board_libraries = workspace::cargo(&workspace.root);
    board_libraries.args(["clippy", "--lib", "--tests"]);
    for board in workspace.boards() {
        board_libraries.args(["--package", &board.name]);
    }
    workspace::execute(board_libraries.args(["--", "-D", "warnings"]))?;

    for board in workspace.boards() {
        workspace::execute(workspace::cargo(&workspace.root).args([
            "clippy",
            "--package",
            &board.name,
            "--package",
            &board.chip_hal,
            "--target",
            &board.target,
            "--",
            "-D",
            "warnings",
        ]))?;
    }
    Ok(())
}

fn board_image(workspace: &Workspace, board: &Board) -> Result<()> {
    let images = firmware::build(workspace, board)?;
    let memory_x_path = board.directory.join("memory.x");
    let memory_x = fs::read_to_string(&memory_x_path)
        .with_context(|| format!("could not read `{}`", memory_x_path.display()))?;
    let layout = MemoryLayout::parse(&memory_x)?;
    let data = fs::read(&images.elf)
        .with_context(|| format!("could not read `{}`", images.elf.display()))?;
    let elf = Elf::parse(&data)?;

    let vectors = elf
        .section(".vector_table")?
        .context("the ELF file has no `.vector_table` section")?;
    if vectors.address != layout.flash.start {
        bail!(
            "the vector table lies at {}, `memory.x` puts `FLASH` at {}",
            Hex(vectors.address),
            Hex(layout.flash.start)
        );
    }
    let stack_top = vectors
        .bytes
        .first_chunk()
        .map(|bytes| u32::from_le_bytes(*bytes));
    if stack_top != Some(layout.ram.end) {
        bail!(
            "the initial stack pointer is {}, `memory.x` puts the end of `RAM` at {}",
            stack_top.map_or_else(|| "missing".to_owned(), |value| Hex(value).to_string()),
            Hex(layout.ram.end)
        );
    }

    let section = elf
        .section(".app_descriptor")?
        .context("the ELF file has no `.app_descriptor` section")?;
    let offset = usize::try_from(section.address - layout.flash.start)?;
    let expected_offset = vectors.bytes.len().next_multiple_of(Descriptor::ALIGNMENT);
    if offset != expected_offset
        || offset > Descriptor::MAX_OFFSET
        || vectors.bytes.len() % VECTOR_SIZE != 0
    {
        bail!(
            "the descriptor lies {offset} bytes into the image, expected {expected_offset} \
             bytes, behind the vector table"
        );
    }
    let descriptor = Descriptor::from_bytes(section.bytes)?;
    if descriptor.image() != layout.flash {
        bail!(
            "the descriptor names the image area {}-{}, `memory.x` puts `FLASH` at {}-{}",
            Hex(descriptor.image().start),
            Hex(descriptor.image().end),
            Hex(layout.flash.start),
            Hex(layout.flash.end)
        );
    }

    for segment in elf.loadable_segments()? {
        let end = u64::from(segment.address) + u64::try_from(segment.bytes.len())?;
        if !layout.flash.contains(segment.address) || end > u64::from(layout.flash.end) {
            bail!(
                "loadable data at {} lies outside `FLASH` of `memory.x`",
                Hex(segment.address)
            );
        }
    }

    let binary = fs::read(&images.bin)
        .with_context(|| format!("could not read `{}`", images.bin.display()))?;
    let image = Image::new(&binary, Some(layout.flash.start))?;
    if *image.descriptor() != descriptor {
        bail!("the descriptor found in the binary differs from the ELF section");
    }
    Ok(())
}

fn relative(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .display()
        .to_string()
}
