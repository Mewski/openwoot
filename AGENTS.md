# openwoot: notes for coding agents

Keyboard firmware for Wooting hall-effect keyboards, written in Rust, with its own configuration protocol shared with a Rust host application. This branch (`rust`) is a new firmware; the branch `main` holds the earlier C firmware, which is the reference for features and hardware behaviour. Read code there; never copy it.

**State: scaffold.** The layout below is in place with empty files; nothing builds yet. Update this file whenever the layout, commands or rules change.

## Layout

| Path | Holds | May depend on |
| --- | --- | --- |
| `crates/openwoot/` | the firmware: scan, analog, keys, layout, HID, USB classes, configuration handler, storage, profiles, lighting, runtime; `no_std`, no hardware access, tested on the host | `openwoot-protocol`, `openwoot-drivers` traits, ecosystem crates |
| `crates/openwoot-protocol/` | messages between firmware and host software (commands, configuration, keymap, lighting); `no_std` | ecosystem crates only |
| `crates/openwoot-drivers/` | chip drivers (`is31fl3741` LED matrix, `spi_nor` flash), generic over `embedded-hal` traits | `embedded-hal` |
| `crates/openwoot-gd32f4/`, `crates/openwoot-gd32f3/` | one HAL per microcontroller family on the GD32 PACs; the only crates with `unsafe` register access | their PAC, `cortex-m`, `embedded-hal` |
| `crates/openwoot-cli/` | host command-line tool: find keyboards, talk the configuration protocol, flash through the factory bootloader | `openwoot-protocol` |
| `boards/<board>/` | one binary crate per keyboard: `memory.x`, `board.json`, `src/board.rs` (the board's numbers, tables and choice of features), `src/main.rs` (wiring) | everything above except the CLI |
| `xtask/` | build helper: list boards, build images, checks, flashing via the CLI | host only |

Optional features (light bar, host signals, editions, ...) and alternative behaviours (noise recalibration methods, device-information storage) are modules with types in `crates/openwoot/`; a board selects them in `src/board.rs`. Shared crates have no Cargo features that depend on a board, and no shared code names a board or a family.

## Rules

- Pure Rust: no C, assembly files, Python, shell scripts or make. Prefer maintained ecosystem crates over own code; justify each new dependency (maintenance, `no_std`, `unsafe`, licence compatible with MIT).
- `unsafe` only in the family HAL crates, each block with a `// SAFETY:` comment.
- No panics on the scan or USB path; no heap.
- The microcontroller flash below the image base holds the factory bootloader, and the pages above the image end hold the serial number and device information. Code must never erase or program either; `memory.x` keeps the linker out of them.
- The USB face (ids, interfaces, HID reports, the analog report read by the Wooting Analog SDK) stays close to stock; deviations are listed in `docs/protocol.md`.
- Nothing is flashed to a keyboard and nothing is pushed without the owner's go-ahead.
- Commits: one line, conventional (`feat(keys): ...`, `fix(usb): ...`, `docs: ...`).

## Code style

- Code documents itself: names, types and small functions carry the meaning. No comments, no doc comments and no commented-out code. The one exception is the `// SAFETY:` comment that every `unsafe` block requires.
- Lints are set once in the workspace `Cargo.toml` (`[workspace.lints]`, clippy pedantic plus denials of panics, `unwrap`, indexing, `todo!` and undocumented `unsafe`); every crate has `[lints] workspace = true`. An `#[allow]` or `#[expect]` must give a `reason`.
- Formatting is enforced, never hand-tuned:
  - Rust: `cargo +nightly fmt` (`rustfmt.toml`; import grouping, import merging and impl item ordering are nightly-only options, so formatting uses nightly while building uses the pinned stable toolchain).
  - TOML: `taplo fmt` (`taplo.toml`); `Cargo.toml` dependency and section order: `cargo sort --workspace`.
- Before every commit: `cargo +nightly fmt --check`, `taplo fmt --check`, `cargo sort --workspace --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Modules are small and named for what they hold; one concept per file; public items first, helpers after.
