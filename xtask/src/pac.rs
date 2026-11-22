mod patches;
mod svd;

use std::{fs, io, path::Path, process::Command};

use anyhow::{Context, Result, ensure};
use svd_rs::Device;

use crate::process;

const SVD2RUST: Tool = Tool {
    program: "svd2rust",
    version: "svd2rust 0.37.1",
};

const FORM: Tool = Tool {
    program: "form",
    version: "form 0.13.0",
};

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
    for chip in &CHIPS {
        chip.generate(root)
            .with_context(|| format!("could not generate the `{}` PAC", chip.name))?;
    }
    Ok(())
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
            reported.starts_with(self.version),
            "`{}` reports `{}`, the PACs are generated with `{}`",
            self.program,
            reported.trim(),
            self.version
        );
        Ok(())
    }
}

struct Chip {
    name: &'static str,
    svd_file: &'static str,
    patch: fn(&mut Device) -> Result<()>,
}

impl Chip {
    fn generate(&self, root: &Path) -> Result<()> {
        let package = root.join("crates").join(format!("{}-pac", self.name));
        let work = root.join("target").join("pac").join(self.name);
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

        let src = package.join("src");
        recreate(&src)?;
        process::execute(
            Command::new(FORM.program)
                .env("RUST_LOG", "warn")
                .arg("--input")
                .arg(work.join("lib.rs"))
                .arg("--outdir")
                .arg(&src),
        )?;
        copy(&work.join("generic.rs"), &src.join("generic.rs"))?;
        for file in ["build.rs", "device.x"] {
            copy(&work.join(file), &package.join(file))?;
        }

        process::execute(
            Command::new("rustfmt")
                .args(["+nightly", "--edition", "2024"])
                .arg(src.join("lib.rs"))
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
