# Building

## Toolchain

`rust-toolchain.toml` pins Rust 1.100.0 with the target `thumbv7em-none-eabihf` (both
microcontrollers are Cortex-M4F). Formatting uses a nightly `rustfmt`
(`rustup toolchain install nightly --component rustfmt`); `cargo xtask check` also needs `taplo`
and `cargo-sort` (`cargo install taplo-cli cargo-sort`).

## Commands

| Command | Does |
| --- | --- |
| `cargo test --workspace` | host tests of every crate, including the board crates' libraries |
| `cargo xtask boards [--json]` | lists the packages under `boards/` |
| `cargo xtask build <board>` / `--all` | `cargo build --release` for the board's target, then writes `build/<board>/openwoot-<board>.elf`, `.hex` and `.bin` |
| `cargo xtask flash <board> --dry-run` | builds the board and runs `openwoot flash --dry-run` on its HEX file |
| `cargo xtask check` | the rules of the tree (below), formatting, clippy, and every board image |
| `cargo xtask pac` | regenerates the PAC crates (needs `svd2rust` and `form`) |

The target of a board comes from `[package.metadata.openwoot] target` of its chip HAL. HEX and
binary files are written in Rust from the loadable segments of the ELF file.

## What `cargo xtask check` checks

- no C, C++, assembly, Python, shell or make files anywhere in the tree;
- dependency direction between the kinds of crates;
- every crate except the PACs inherits the workspace lints;
- the chip HAL crates have the same top-level files (a peripheral module's children are
  chip-specific), and the board crates have the same file set;
- the shared crates (`openwoot`, `openwoot-protocol`, `openwoot-drivers`, `openwoot-image`) name
  no board and no chip;
- outside the PACs: no comments except `// SAFETY:` notes, no modules named `utils`, `misc`,
  `helpers`, `common` or `types`, `unsafe` only in chip HALs and each with a `// SAFETY:` note;
- `cargo +nightly fmt --check`, `taplo fmt --check`, `cargo sort --workspace --check`;
- clippy with `-D warnings` on the host and for every board on its target;
- every board image: vector table at the FLASH origin of `memory.x`, initial stack pointer at the
  end of RAM, descriptor on the 64-byte boundary behind the vector table and equal to `memory.x`,
  loadable data inside FLASH, and the host tool's image checks on the binary;
- no panic reachable from an interrupt vector or the key task (`scanning`), found on the call
  graph built from the image's relocations; panic sites of pinned dependencies that cannot fire
  are listed with a reason in `xtask/src/check/panic_paths/reviewed.rs`, and a listed site that no
  image contains is reported;
- per board, statics plus the stack bound of the call graph (thread mode plus every interrupt
  handler and the key task) leave 2048 bytes of RAM, and the image leaves 32768 bytes of its
  area; the budget is printed for every board.
