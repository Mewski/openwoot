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
| `cargo xtask build <board>` / `--all` | `cargo build --release` of the board's firmware package for the board's target, then writes `build/<board>/openwoot-<board>.elf`, `.hex` and `.bin`, and copies the link map to `.map` |
| `cargo xtask flash <board> --dry-run` | builds the board and runs `openwoot flash --dry-run` on its HEX file |
| `cargo xtask check` | the rules of the tree (below), formatting, clippy, and every board image |
| `cargo xtask pac` | regenerates the PAC crates (needs exactly `svd2rust` 0.37.1 and `form` 0.13.0); CI runs it and fails if the committed PACs differ |

The target of a board comes from `[package.metadata.openwoot] target` of its chip HAL. HEX and
binary files are written in Rust from the loadable segments of the ELF file.

## Speed and size

Each board is two packages: the board support crate `boards/<board>/` and its firmware binary
`openwoot-<board>` in `boards/<board>/firmware/`. Nearly all of the firmware is generic over the
board's hardware, so its code is generated in whichever of the two crates names the concrete
types, and a crate's `opt-level` applies to that code. The release profile builds at
`opt-level = 2`, as the stock firmware is built with GCC `-O2`; at higher levels the Wooting
60HE+'s code no longer ends below its zero-wait limit `0x0804_0000`. The workspace
`Cargo.toml` lists every board support crate with `opt-level = "z"`. The board support crate
instantiates the configuration side (settings storage, profile model, configuration channel and
profile tasks, start-up with `Board::new`, which takes `&dyn Interrupts` so that it is not
generic and stays in this crate) and the lighting task, so these are compiled for size; the
firmware binary binds the interrupt vectors and runs the key and USB tasks (`scanning`, `device`,
`reports`), so that code, and every shared function they reach, is compiled for speed, and as the
final crate of the link the binary sets the optimisation of the whole link-time pass. Lighting
runs on the lowest interrupt executor, where size-compiled code still renders every 16 ms
(62.5 Hz). `xtask check` reads the crate of every function from the image's DWARF compile units
and rejects code that an interrupt vector or a task of the key or device executor reaches in a
crate compiled for size.

## Zero-wait flash

Both microcontrollers fetch code without wait states only from the start of their flash
(`ZERO_WAIT` in `memory.x`: 512 KiB on the GD32F425, 256 KiB on the GD32F303); beyond it every
fetch waits. The firmware is linked in the compiler's order, the same for `cargo build` and
`xtask build`; nothing reorders it. `xtask check` requires all code (`.vector_table` and `.text`)
to lie below the end of `ZERO_WAIT`, as the stock firmware's code does; constants may extend
beyond it. The firmware's `build.rs` also has the linker write a map (`-Map`); `xtask build`
finds it and the ELF file in cargo's JSON messages and copies it next to the image, and the
checks below read its input sections to give every anonymous constant and vtable its own node.

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
- no panic reachable from an interrupt vector or a task of an interrupt executor (`scanning` on the
  key executor, `device` and `reports` on the device executor, `lights` on the lighting executor,
  found by their names), on the call graph built from the image's relocations; an image without
  them, or with a `bl` that has none, is rejected. A `MOVW`/`MOVT` address is joined along the code
  path from the `MOVW`, following unconditional branches, so several `MOVW`s may share one `MOVT`
  if they name the same symbol and lower half; any other unpaired half is rejected. The graph
  follows calls and the addresses taken in code and tables, not calls through pointers stored at
  run time (wakers, `dyn` objects registered at run time), and it leaves out the panic of an
  `async fn` resumed after completion. Thread-mode tasks (`channel`, `profiles`) are not checked.
  Panic sites of pinned dependencies that cannot fire are listed with a reason and the contexts
  they are reached from (`reached_from`: `Origin::Interrupt`, `Origin::Task(Executor::..)`) in
  `xtask/src/check/panic_paths/reviewed.rs`. A panic call is waived only when every `Location`
  that can reach it lies in the cargo registry and is listed: the one loaded within 64 bytes
  before the call and after the last call or unconditional branch and, since the compiler merges
  panic calls behind one shared block, the ones loaded in the blocks that jump into that block,
  level by level. A listed site that no image contains is reported;
- no BASEPRI write, so no critical section, reachable from the scan vector (`ADC` on the GD32F425,
  `ADC0_1` on the GD32F303);
- the vector table and all of `.text` lie inside `ZERO_WAIT` of `memory.x`, and no function that a
  device interrupt vector (the vectors from 16 on) or a task of the key or device executor reaches
  belongs to a crate that the release profile compiles for size (`opt-level` `"s"` or `"z"`; the
  crate of a function is the crate of its DWARF compile unit, the crate that instantiated it); the
  lighting executor is size-compiled by design and not checked;
- per board, statics (from the start of RAM to the end of the last static section) plus the stack
  bound of the call graph (thread mode, plus the deepest task of each interrupt executor, plus
  every interrupt handler) leave 2048 bytes of RAM, and the image leaves 32768 bytes of its area;
  calls that form a cycle are rejected, functions joined only through tables count once, and the
  budget is printed for every board (`RAM N bytes static + N bytes stack (N thread, N keys,
  N device, N lighting, N interrupts)`).
