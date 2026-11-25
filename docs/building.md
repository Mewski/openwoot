# Building

## Toolchain

`rust-toolchain.toml` pins Rust 1.100.0 with the target `thumbv7em-none-eabihf` (both
microcontrollers are Cortex-M4F). Formatting uses a nightly `rustfmt`
(`rustup toolchain install nightly --component rustfmt`); `xtask` runs `nightly`, or the toolchain
named in `OPENWOOT_NIGHTLY`, which CI sets to the dated nightly it installs. `cargo xtask check`
also needs `taplo` and `cargo-sort` (`cargo install taplo-cli@0.10.0 cargo-sort@2.1.5`, the
versions CI installs).

On Linux, `openwoot-gui` (built by `cargo test --workspace` and `cargo xtask check` too) needs the
development files of fontconfig, Wayland and xkbcommon; on Debian and Ubuntu
`apt install libfontconfig-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev`, the packages
CI installs. macOS needs nothing beyond the Xcode command line tools.

## Commands

| Command | Does |
| --- | --- |
| `cargo test --workspace` | host tests of every crate, including the board crates' libraries |
| `cargo xtask boards [--json]` | lists the packages under `boards/` |
| `cargo xtask build <board>` / `--all` | `cargo build --release` for the board's target in link order (below), then writes `build/<board>/openwoot-<board>.elf`, `.hex` and `.bin` |
| `cargo xtask flash <board> --dry-run` | builds the board and runs `openwoot flash --dry-run` on its HEX file |
| `cargo xtask check` | the rules of the tree (below), formatting, clippy, and every board image |
| `cargo xtask pac` | regenerates the PAC crates (needs exactly `svd2rust` 0.37.1 and `form` 0.13.0); CI runs it and fails if the committed PACs differ |

The target of a board comes from `[package.metadata.openwoot] target` of its chip HAL. HEX and
binary files are written in Rust from the loadable segments of the ELF file.

## Link order

Both microcontrollers fetch code without wait states only from the start of their flash
(`ZERO_WAIT` in `memory.x`: 512 KiB on the GD32F425, 256 KiB on the GD32F303); beyond it every
fetch waits. The images are larger, and the compiler's order of functions is arbitrary, so
`xtask` builds each board twice when needed: it links the image, builds the call graph from its
relocations and writes `target/<target>/release/openwoot-<board>.symbol-order`, which lists the
functions that a device interrupt vector or the key task reaches, then the thread executor, then
the code of each thread task, the task with the least new code first, each group sorted by
name. When the list differs from the one the image was linked with, it links again. The board's
`build.rs` passes the file named by `OPENWOOT_SYMBOL_ORDER` to lld as `--symbol-ordering-file`,
which puts those functions first in `.text`. A plain `cargo build` links in the compiler's order;
build and flash images through `xtask`.

## What `cargo xtask check` checks

- no C, C++, assembly, Python, shell or make files anywhere in the tree except `target/`, `build/`
  and the hidden directories at the root other than `.cargo` and `.github`;
- dependency direction between the kinds of workspace crates, each chip HAL on exactly one PAC and
  each board on no PAC but its chip HAL's (dependencies on ecosystem crates are not checked);
- every crate except the two PACs that `cargo xtask pac` generates inherits the workspace lints;
- the chip HAL crates have the same top-level files (a peripheral module's children are
  chip-specific), and the board crates have the same file set;
- the shared crates (`openwoot`, `openwoot-protocol`, `openwoot-drivers`, `openwoot-image`) contain
  none of the names derived from the packages: the board package names (also with `_`), the chip
  HAL names without `openwoot-` and the PAC names without `-pac`; other spellings, such as `80he`,
  are not caught;
- outside the PACs: no comments except `// SAFETY:` notes, no modules named `utils`, `misc`,
  `helpers`, `common` or `types`, `unsafe` only in chip HALs and each with a `// SAFETY:` note on
  its line or directly above it (attributes may stand between); every note gives its reason on its
  first line and stands on or above `unsafe` code;
- `cargo +nightly fmt --check`, `taplo fmt --check`, `cargo sort --workspace --check`;
- clippy with `-D warnings` on the host and for every board on its target;
- every board image: vector table at the FLASH origin of `memory.x`, initial stack pointer at the
  end of RAM, the 72-byte descriptor on the 64-byte boundary behind the vector table, naming the
  image area and RAM of `memory.x`, loadable data inside FLASH, and the host tool's image checks on
  the binary (which compare its descriptor, update protocol included, with the ELF section);
- no panic reachable from an interrupt vector or the key task (`scanning`, found by its name), on
  the call graph built from the image's relocations; an image without them, or with a `bl` that
  has none, is rejected. A `MOVW`/`MOVT` address is joined along the code path from the `MOVW`,
  following unconditional branches, so several `MOVW`s may share one `MOVT` if they name the same
  symbol and lower half; any other unpaired half is rejected. The graph follows calls and the addresses taken in code and tables, not
  calls through pointers stored at run time (wakers, `dyn` objects registered at run time), and it
  leaves out the panic of an `async fn` resumed after completion. Thread-mode tasks are not
  checked. Panic sites of pinned dependencies that cannot fire are listed with a reason and the
  context they are reached from (interrupts or the key task) in
  `xtask/src/check/panic_paths/reviewed.rs`; a site matches only when its `Location` is loaded
  after the last call before the panic and lies in the cargo registry, and a listed site that no
  image contains is reported;
- no BASEPRI write, so no critical section, reachable from the scan vector (`ADC` on the GD32F425,
  `ADC0_1` on the GD32F303);
- every function that a device interrupt vector (the vectors from 16 on; the core exceptions end
  in `cortex-m-rt`'s handlers, whose `HardFault` lies behind `.text`) or the key task reaches lies
  inside `ZERO_WAIT` of `memory.x`;
- per board, statics (from the start of RAM to the end of the last static section) plus the stack
  bound of the call graph (thread mode plus every interrupt handler and the key task) leave 2048
  bytes of RAM, and the image leaves 32768 bytes of its area; calls that form a cycle are
  rejected, functions joined only through tables count once, and the budget is printed for every
  board.
