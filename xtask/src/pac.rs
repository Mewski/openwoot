mod patches;
mod svd;

use std::{fs, io, path::Path, process::Command};

use anyhow::{Context, Result, ensure};
use svd_rs::Device;

use crate::{process, workspace::Workspace};

const SVD2RUST: Tool = Tool {
    program: "svd2rust",
    version: "0.37.1",
};

const FORM: Tool = Tool {
    program: "form",
    version: "0.13.0",
};

const PACKAGE_SUFFIX: &str = "-pac";

const CHIPS: [Chip; 2] = [
    Chip {
        name: "gd32f425",
        svd_file: "GD32F4xx.svd",
        patch: patches::gd32f425,
    },
    Chip {
        name: "gd32f303",
        svd_file: "GD32F30x_XD.svd",
        patch: patches::gd32f303,
    },
];

pub(crate) fn run(root: &Path) -> Result<()> {
    SVD2RUST.require()?;
    FORM.require()?;
    let workspace = Workspace::load(root)?;
    for chip in &CHIPS {
        chip.generate(root, workspace.target_directory())
            .with_context(|| format!("could not generate the `{}` PAC", chip.name))?;
    }
    Ok(())
}

pub(crate) fn is_generated(package: &str) -> bool {
    package
        .strip_suffix(PACKAGE_SUFFIX)
        .is_some_and(|chip| CHIPS.iter().any(|known| known.name == chip))
}

struct Tool {
    program: &'static str,
    version: &'static str,
}

impl Tool {
    fn require(&self) -> Result<()> {
        let output = Command::new(self.program)
            .arg("--version")
            .output()
            .with_context(|| format!("could not run `{} --version`", self.program))?;
        ensure!(
            output.status.success(),
            "`{} --version` ended with {}",
            self.program,
            output.status
        );
        let reported = String::from_utf8_lossy(&output.stdout);
        ensure!(
            reports_version(&reported, self.program, self.version),
            "`{}` reports `{}`, the PACs are generated with version {}",
            self.program,
            reported.trim(),
            self.version
        );
        Ok(())
    }
}

fn reports_version(reported: &str, program: &str, version: &str) -> bool {
    let mut words = reported.split_whitespace();
    words.next() == Some(program) && words.next() == Some(version)
}

struct Chip {
    name: &'static str,
    svd_file: &'static str,
    patch: fn(&mut Device) -> Result<()>,
}

impl Chip {
    fn generate(&self, root: &Path, target: &Path) -> Result<()> {
        let package = root
            .join("crates")
            .join(format!("{}{PACKAGE_SUFFIX}", self.name));
        let work = target.join("pac").join(self.name);
        recreate(&work)?;

        let source = package.join("svd").join(self.svd_file);
        let xml = fs::read_to_string(&source)
            .with_context(|| format!("could not read `{}`", source.display()))?;
        let mut device = svd_parser::parse(&svd::with_standard_access(&xml))
            .with_context(|| format!("could not parse `{}`", source.display()))?;
        (self.patch)(&mut device)?;
        let patched = work.join(format!("{}.svd", self.name));
        fs::write(&patched, svd_encoder::encode(&device)?)
            .with_context(|| format!("could not write `{}`", patched.display()))?;

        process::execute(
            Command::new(SVD2RUST.program)
                .args([
                    "--target",
                    "cortex-m",
                    "--edition",
                    "2024",
                    "--generic-mod",
                    "--reexport-interrupt",
                    "--log",
                    "warn",
                ])
                .arg("-i")
                .arg(&patched)
                .arg("--output-dir")
                .arg(&work),
        )?;

        let formed = work.join("src");
        recreate(&formed)?;
        process::execute(
            Command::new(FORM.program)
                .env("RUST_LOG", "warn")
                .arg("--input")
                .arg(work.join("lib.rs"))
                .arg("--outdir")
                .arg(&formed),
        )?;
        copy(&work.join("generic.rs"), &formed.join("generic.rs"))?;

        let sources = package.join("src");
        recreate(&sources)?;
        copy_tree(&formed, &sources)?;
        for file in ["build.rs", "device.x"] {
            copy(&work.join(file), &package.join(file))?;
        }

        process::execute(
            Command::new("rustfmt")
                .arg(process::nightly())
                .args(["--edition", "2024"])
                .arg(sources.join("lib.rs"))
                .arg(package.join("build.rs"))
                .current_dir(root),
        )
    }
}

fn recreate(directory: &Path) -> Result<()> {
    match fs::remove_dir_all(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("could not remove `{}`", directory.display()));
        }
    }
    fs::create_dir_all(directory)
        .with_context(|| format!("could not create `{}`", directory.display()))
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    fs::copy(from, to)
        .with_context(|| format!("could not copy `{}` to `{}`", from.display(), to.display()))?;
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("could not create `{}`", to.display()))?;
    let unreadable = || format!("could not read `{}`", from.display());
    for entry in fs::read_dir(from).with_context(unreadable)? {
        let entry = entry.with_context(unreadable)?;
        let (source, destination) = (entry.path(), to.join(entry.file_name()));
        if source.is_dir() {
            copy_tree(&source, &destination)?;
        } else {
            copy(&source, &destination)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn only_the_generated_crates_count_as_pacs() {
        assert!(is_generated("gd32f425-pac"));
        assert!(is_generated("gd32f303-pac"));
        assert!(!is_generated("gd32f405-pac"));
        assert!(!is_generated("gd32f425"));
        assert!(!is_generated("openwoot-gd32f4"));
    }

    #[test]
    fn a_tool_must_report_exactly_the_pinned_version() {
        assert!(reports_version(
            "svd2rust 0.37.1 (8f7c2a1 2025-06-01)\n",
            "svd2rust",
            "0.37.1"
        ));
        assert!(reports_version("form 0.13.0\n", "form", "0.13.0"));
        assert!(!reports_version("svd2rust 0.37.10\n", "svd2rust", "0.37.1"));
        assert!(!reports_version("form 0.12.0\n", "form", "0.13.0"));
        assert!(!reports_version("", "form", "0.13.0"));
    }

    #[test]
    fn a_tree_is_copied_with_its_subdirectories() {
        let root = env::temp_dir().join(format!("openwoot-xtask-pac-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let (from, to) = (root.join("formed"), root.join("src"));
        fs::create_dir_all(from.join("adc0")).unwrap();
        fs::write(from.join("lib.rs"), "mod adc0;").unwrap();
        fs::write(from.join("adc0").join("ctl0.rs"), "pub struct Ctl0;").unwrap();
        copy_tree(&from, &to).unwrap();
        let copied = (
            fs::read_to_string(to.join("lib.rs")).unwrap(),
            fs::read_to_string(to.join("adc0").join("ctl0.rs")).unwrap(),
        );
        fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            copied,
            ("mod adc0;".to_owned(), "pub struct Ctl0;".to_owned())
        );
    }
}
