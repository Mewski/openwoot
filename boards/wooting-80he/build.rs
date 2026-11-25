use std::{env, error::Error, fs, path::PathBuf};

use openwoot_image::MemoryLayout;

const SYMBOL_ORDER: &str = "OPENWOOT_SYMBOL_ORDER";

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-env-changed={SYMBOL_ORDER}");
    let memory_x = fs::read_to_string("memory.x")?;
    let layout = MemoryLayout::parse(&memory_x)?;

    let out = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?);
    fs::write(out.join("memory.x"), &memory_x)?;
    fs::write(
        out.join("memory_layout.rs"),
        format!(
            "pub const IMAGE: Region = Region {{ start: {}, end: {} }};\npub const RAM: Region = \
             Region {{ start: {}, end: {} }};\n",
            grouped_hex(layout.flash.start),
            grouped_hex(layout.flash.end),
            grouped_hex(layout.ram.start),
            grouped_hex(layout.ram.end),
        ),
    )?;
    println!("cargo:rustc-link-search={}", out.display());

    if env::var("TARGET")?.starts_with("thumb") {
        println!("cargo:rustc-link-arg-bins=--nmagic");
        println!("cargo:rustc-link-arg-bins=-Tlink.x");
        println!("cargo:rustc-link-arg-bins=--emit-relocs");
        if let Some(order) = env::var_os(SYMBOL_ORDER).map(PathBuf::from) {
            println!("cargo:rerun-if-changed={}", order.display());
            println!(
                "cargo:rustc-link-arg-bins=--symbol-ordering-file={}",
                order.display()
            );
            println!("cargo:rustc-link-arg-bins=--no-warn-symbol-ordering");
        }
    }
    Ok(())
}

fn grouped_hex(value: u32) -> String {
    format!("0x{:04X}_{:04X}", value >> 16, value & 0xFFFF)
}
