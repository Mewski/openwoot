use std::{env, error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    if env::var("TARGET")?.starts_with("thumb") {
        let link_map = PathBuf::from(env::var("OUT_DIR")?).join("link.map");
        println!("cargo:rustc-link-arg-bins=--nmagic");
        println!("cargo:rustc-link-arg-bins=-Tlink.x");
        println!("cargo:rustc-link-arg-bins=--emit-relocs");
        println!("cargo:rustc-link-arg-bins=-Map={}", link_map.display());
    }
    Ok(())
}
