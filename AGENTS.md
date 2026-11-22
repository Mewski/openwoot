# openwoot: notes for coding agents

Keyboard firmware for Wooting hall-effect keyboards, written in Rust, with its own configuration protocol shared with a Rust host application. This branch (`rust`) is a new firmware; the branch `main` holds the earlier C firmware, which is the reference for features and hardware behaviour. Read code there; never copy it.

**State: milestone M1 reached on hardware (Wooting 80HE).** The image enumerates at 480 Mbit/s with the stock-like face and the board's serial number, `openwoot version` answers over the configuration channel, and the official Wootility recognises the keyboard, requires its update and returns it to stock firmware through the factory bootloader. Done: generated PACs, image descriptor, chip bring-up on both families (clocks, GPIO, time, interrupts, executors, update-mode hand-over, read-only flash), USB device-controller drivers on both families (the 60HE+ never run on hardware), the USB face with descriptors for the negotiated speed, the configuration channel (`GetVersion`, `EnterUpdateMode`), the stock identification responder, the runtime (USB device, channel and idle HID report tasks), the CLI (`version`, `flash`). Not yet: scanning, keys, storage, lighting, profiles. Update this file whenever the layout, commands or rules change.

## Layout

| Path | Holds | May depend on |
| --- | --- | --- |
| `crates/openwoot/` | the firmware: the `Keyboard` trait a board implements, `hid` (descriptor builder, typed reports), `usb` (the USB face over any `embassy_usb_driver::Driver`), `protocol` (configuration channel), runtime; later keys, layout, storage, profiles, lighting; `no_std`, no hardware access, tested on the host | `openwoot-protocol`, `openwoot-drivers`, `openwoot-image`, ecosystem crates |
| `crates/openwoot-protocol/` | the configuration protocol between firmware and host software: postcard-rpc endpoints (`GetVersion`, `EnterUpdateMode`), HID report framing, `stock` (the stock feature-command framing shared by the firmware's identification responder and the CLI); `no_std` | ecosystem crates only |
| `crates/openwoot-image/` | the firmware image format: the image descriptor, its location, the image checks, the `memory.x` reader; `no_std`; used by boards, the CLI and `xtask` | ecosystem crates only |
| `crates/openwoot-drivers/` | chip drivers (`is31fl3741` LED matrix, `spi_nor` flash), generic over `embedded-hal-async` traits | `embedded-hal-async` |
| `crates/gd32f425-pac/`, `crates/gd32f303-pac/` | generated register access (`cargo xtask pac`); exempt from the comment and `unsafe` rules | `cortex-m`, `cortex-m-rt`, `vcell` |
| `crates/openwoot-gd32f4/`, `crates/openwoot-gd32f3/` | one HAL per microcontroller family on its PAC (`rt` only for `target_os = "none"`): `rcu`, `gpio`, `time` (embassy-time driver), `interrupt` (`bind_interrupts!`), `executor`, `system` (reset, update-mode hand-over), `usb` (`usb::Driver`, `usb::negotiated_speed()`; the GD32F425 wraps `embassy-usb-synopsys-otg` with the ULPI PHY, the GD32F303 has a port of embassy-stm32's fsdev driver under `src/usb/`); private `basepri` (critical section) and `register`; `app_descriptor!`; the only hand-written crates with `unsafe` (`#![allow(unsafe_code, reason = ...)]`) | their PAC, `openwoot`, `openwoot-image`, `cortex-m`, `embedded-hal`, `embassy-usb-driver`, `embassy-sync`, `embassy-time`, `embassy-usb-synopsys-otg` |
| `crates/openwoot-cli/` | host tool `openwoot` (`version`, `flash [--dry-run]`): image checks, stock enter-update-mode command, factory bootloader session with CRC-24 verification, configuration channel; `async-hid` with `async-io`, `indicatif` | `openwoot-protocol`, `openwoot-image` |
| `boards/<board>/` | one board support crate per keyboard (see Boards) | everything above except the CLI |
| `xtask/` | build helper: list boards, build images, checks, flashing via the CLI, PAC generation | `openwoot-image` |

Optional features (light bar, host signals, editions, ...) and alternative behaviours (noise recalibration methods, device-information storage) are modules with types in `crates/openwoot/`; a board selects them in its library. Shared crates have no Cargo features that depend on a board, and no shared code names a board or a family.

## Boards

- A board is a board support crate `boards/<board>/`, kebab-case, package name = directory name: a library with the board facts as typed constants (name, USB ids, update mode, protected flash areas, key matrix), `DESCRIPTOR`, `pub struct Board` (later: the configured drivers and peripherals by board function, built by `Board::new(peripherals)`) with `impl openwoot::Keyboard`, re-exports `hal` (its chip HAL) and `entry`; the binary `openwoot-<board>` in `src/main.rs`; `memory.x` and `build.rs` as board support crates ship them.
- `memory.x` is the single source of the image area and RAM, and places the image descriptor (section `.app_descriptor`) on the first 64-byte boundary behind the vector table; `build.rs` reads it into the constant `IMAGE`.
- The image describes itself: the descriptor (`openwoot_image::Descriptor`) carries the board name, USB ids, update mode and image area. There is no `board.json`; the CLI never lists boards, and `xtask` finds them under `boards/`.

## Real time

- Interrupt priorities: 0 the hall-sensor scan only (never masked), 1 the time driver, 2 USB, 3 the key executor (on the UART4 vector). Critical sections raise BASEPRI to `0x10` and never mask the scan interrupt.
- TIMER4 belongs to the time driver, UART4's vector to the key executor.
- `memory.x` aligns `_stext` to 8 bytes behind the descriptor.

## Commands

| Command | Does |
| --- | --- |
| `cargo test --workspace` | host tests of every crate |
| `cargo xtask boards [--json]` | lists the boards |
| `cargo xtask build <board>` / `--all` | `build/<board>/openwoot-<board>.{elf,hex,bin}` |
| `cargo xtask flash <board> --dry-run` | builds and checks the image with `openwoot flash --dry-run`; without `--dry-run` it would flash, which needs the owner's go-ahead |
| `cargo xtask check` | file types, dependency direction, file-set symmetry (boards: every file; chip HALs: top-level `src/*.rs`, since a peripheral module's children are chip-specific), names in shared code, comment, `unsafe` and module-name rules, formatting, clippy (host and every board target), every board image |
| `cargo xtask pac` | regenerates the PAC crates |
| `openwoot version` | firmware and protocol version over the configuration channel |
| `openwoot flash <image> [--dry-run]` | checks the image; enters update mode (openwoot: `EnterUpdateMode`; stock: feature command 2), writes through the factory bootloader with CRC-24 verification; `--dry-run` only checks |

## Rules

- Pure Rust: no C, assembly files, Python, shell scripts or make. Prefer maintained ecosystem crates over own code; justify each new dependency (maintenance, `no_std`, `unsafe`, licence compatible with MIT). Exact version pins live in `[workspace.dependencies]`, added when a crate first uses them (cargo warns about unused entries).
- `unsafe` only in the chip HAL crates (and the generated PACs), each with a `// SAFETY:` comment.
- No panics on the scan or USB path; no heap.
- The microcontroller flash below the image base holds the factory bootloader, and the pages above the image end hold the serial number and device information. Code must never erase or program either; `memory.x` keeps the linker out of them and each board tests its image area against them.
- The USB face (ids, interfaces, HID reports, the analog report read by the Wooting Analog SDK) stays close to stock; deviations are listed in `docs/protocol.md`.
- Nothing is flashed to a keyboard and nothing is pushed without the owner's go-ahead.
- Commits: one line, conventional (`feat(keys): ...`, `fix(usb): ...`, `docs: ...`).

## Naming

- Rust API Guidelines naming: RFC 430 casing; acronyms are one word (`UsbIds`, `Hid`, not `USB`); constructors `new`/`with_*`, conversions `from_*`; `as_`/`to_`/`into_` by cost; getters without `get_`; iterators `iter`/`iter_mut`/`into_iter`; no `-rs`/`-rust` in crate names; Cargo features without `use-`/`with-`.
- Names say what a thing is in the domain (`actuation_point`, not `ap`, `val`, `data`, `tmp`, `info`, `mgr`, `util`, `helper`, `common`); no modules named `utils`, `misc`, `helpers`, `common` or `types` (`xtask check` rejects them).
- No stuttering: `keys::Index`, not `keys::KeyIndex`.
- Enforced by clippy (`pedantic` with `module_name_repetitions`, `min_ident_chars`, `renamed_function_params`, `pub_underscore_fields`, `same_name_method`).

## Messages and strings

One voice everywhere a person reads text: errors, CLI output, help, `xtask` output, logs, panic messages, docs examples.

- Errors (`Display` of error types, `anyhow` context, `bail!`): lowercase first word, no trailing period, say what failed and with what (`could not read \`image.hex\``, `reset vector 0x0800_0000 lies outside the image`). Context is added at each layer with `with_context`, and printed as one chain with `{error:#}` (`openwoot: could not read \`image.hex\`: No such file or directory`). Never `Error:`/`error:` prefixes inside messages and never `failed to`; prefer `could not <verb>`.
- Error enums derive their messages with `thiserror` `#[error("...")]`, same rules; variants name the condition (`ResetVectorOutsideImage`), not the check.
- Help text (clap `about`/`help`): sentence case, no trailing period, imperative (`Write a firmware image through the factory bootloader`).
- CLI output: results to stdout as aligned `label: value` lines (labels lowercase, padded to one width per command), progress and diagnostics to stderr; every program prefixes its own fatal error with its name (`openwoot: ...`, `xtask: ...`).
- Identifiers, paths, commands and values from input appear in backticks in messages (`` `wooting-80he` ``, `` `--dry-run` ``).
- Numbers: addresses and register values as lowercase hex with `0x` and `_` every four digits (`0x0800_8000`); USB ids as `31e3:1400`; sizes with their unit spelled out (`3 blocks of 256 bytes`, `768 bytes`); durations with units (`250 ms`).
- Names of products as their makers write them (`Wooting 80HE`, `Wooting 60HE+`, `GD32F425`); everything else lowercase in running text.
- All user-facing strings of one kind live next to the type they describe (error enums, one `show`/output function per binary), never scattered `println!` with ad-hoc formats.

## Code style

- Code documents itself: names, types and small functions carry the meaning. No comments, no doc comments and no commented-out code. The one exception is the `// SAFETY:` comment that every `unsafe` block or attribute requires.
- Lints are set once in the workspace `Cargo.toml` (`[workspace.lints]`, clippy pedantic plus denials of panics, `unwrap`, indexing, `todo!` and undocumented `unsafe`); every crate except the generated PACs has `[lints] workspace = true`. An `#[allow]` or `#[expect]` must give a `reason`.
- Formatting is enforced, never hand-tuned:
  - Rust: `cargo +nightly fmt` (`rustfmt.toml`; import grouping, import merging and impl item ordering are nightly-only options, so formatting uses nightly while building uses the pinned stable toolchain).
  - TOML: `taplo fmt` (`taplo.toml`); `Cargo.toml` dependency and section order: `cargo sort --workspace`.
- Before every commit: `cargo xtask check` and `cargo test --workspace`.
- Modules use the 2018+ file layout: a module `keys` with children is `src/keys.rs` plus `src/keys/actuation.rs`; no `mod.rs` anywhere (clippy `mod_module_files` is denied).
- Every crate and every module with children is a facade: child modules are private (`mod actuation;`), and the parent re-exports the public API (`pub use actuation::{ActuationPoint, Actuation};`). Callers write `openwoot::keys::ActuationPoint`, never `openwoot::keys::actuation::ActuationPoint`. Items shared only inside the crate are `pub(crate)`; `unreachable_pub` warns about anything public that the facade does not export.
- Small modules named for what they hold, one concept per file; public items first, private helpers after.

## Tests

Follows the Rust Book (ch. 11.3) for unit tests and matklad's "Delete Cargo Integration Tests" for integration tests:

- Unit tests live in the file they test, in `#[cfg(test)] mod tests { use super::*; ... }` at the end of the file.
- Integration tests: one integration test crate per package, `tests/it/main.rs`, declaring one module per behaviour area (`tests/it/keys.rs`, `tests/it/flash.rs`) and shared setup as a plain module (`tests/it/support.rs`). One binary links faster than one per file, and no `mod.rs` is needed.
- Test names state the behaviour (`rapid_trigger_releases_after_upstroke`). No test-only code outside test modules and `tests/`.
