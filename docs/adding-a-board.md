# Adding a board

A board is a board support crate in `boards/<board>/` (kebab-case, the package name equals the
directory name) with its firmware binary in `boards/<board>/firmware/` (package and binary
`openwoot-<board>`). Nothing else in the tree lists boards, except the release profile in the
workspace `Cargo.toml`, which compiles each board support crate for size
(`[profile.release.package.<board>] opt-level = "z"`, see [speed and size](building.md#speed-and-size));
the workspace, `xtask` and CI find boards under `boards/`, and the host tool reads everything it
needs from the image descriptor.

## Files

| File | Content |
| --- | --- |
| `Cargo.toml` | package `<board>`, a library; dependencies on exactly one chip HAL, on `openwoot`, `openwoot-drivers`, `openwoot-image`, `openwoot-protocol`, `cortex-m-rt`, `embassy-futures`, `embassy-sync` and `static_cell`, for `target_os = "none"` also on `embassy-executor`, and the build dependency `openwoot-image` (copy an existing board's manifest) |
| `memory.x` | the `FLASH` and `RAM` regions, the `ZERO_WAIT` region (no attributes: the flash that the chip fetches code from without waiting, from `0x0800_0000`; `xtask check` requires all code to lie inside it) and the `.app_descriptor` section behind the vector table (copy an existing board and change the regions) |
| `build.rs` | the same in every board: reads `memory.x` and generates the constants `IMAGE` and `RAM`, and puts `memory.x` on the linker's search path |
| `src/lib.rs` | the board facts as constants (name, USB ids, update mode with its `UpdateProtocol`, or `None` while the bootloader's protocol is unknown, protected flash areas, key matrix), a constant assertion that the image area of `memory.x` starts at the end of the bootloader and ends at the serial page, `DESCRIPTOR`, the `Interrupts` trait (the interrupt bindings the peripherals need), `Board` (the configured drivers by board function, built by the `#[inline(never)]` function `Board::new(peripherals, irqs: &dyn Interrupts)`, which also opens the settings storage; it is not generic, so it is compiled here, for size), re-exports `hal` and `entry`, and tests of the facts |
| `src/scan.rs`, `src/lighting.rs`, `src/storage.rs` | the data of each area: sensor wiring and maps, LED maps and variants, flash layout with the sources of the device information and the layout variant (`DEVICE_INFO`, `LAYOUT_VARIANT`) |
| `src/keys.rs`, `src/profiles.rs` | the factory keymap of each layer with the patches of the layout variants (served by `GetDefaultKeymap`), the key geometry of each layout variant (`GEOMETRY`, served by `GetKeyGeometry`), the capabilities reported to the host, and the factory profiles (`FACTORY`, and an `EDITION` where the board has one) |
| `src/configuration.rs` | the configuration side, compiled for size: the `runtime::State` the tasks share, the `#[inline(never)]` function `start` that builds the `runtime::Firmware`, the configuration handler and the profile model (`open_model`, with the layout variant in effect and the unit's own one: `Storage::layout_variant`, `Model::with_factory_variant(Storage::factory_layout_variant)`) and hands back the parts of every task (`Tasks`), `hand_over` (a static `Signal` that passes the configuration channel from `device` to thread mode), the thread-mode tasks `channel` and `profiles`, and the `lights` task of the lighting executor |
| `firmware/Cargo.toml`, `firmware/build.rs` | the package `openwoot-<board>` with the binary of the same name (`test = false`), depending on the board support crate and `openwoot`, for `target_os = "none"` on `embassy-executor`; the build script passes `--nmagic`, `-Tlink.x`, `--emit-relocs` and `-Map={OUT_DIR}/link.map` to the linker for thumb targets. `--emit-relocs` is required: `cargo xtask check` builds its call graph from the relocations of the image, and without them `xtask` rejects the image; `xtask build` copies the map to `build/<board>/` and the checks read its input sections |
| `firmware/src/main.rs` | the speed side: `hal::bind_interrupts!` with every vector the board uses, `hal::app_descriptor!(DESCRIPTOR)`, the `#[entry]` function, which only starts the executors (`hal::executor::run(start)`), the `#[inline(never)]` function `start` that builds the `Board` (`Board::new(peripherals, &Irqs)`), calls `configuration::start` and spawns each task on its executor (`scanning` on the key executor, `device` on the device executor, `lights` on the lighting executor, `channel` and `profiles` on the thread executor), the time-critical tasks `scanning`, `device` (builds the USB device, hands the configuration channel over and spawns `reports` on its own executor through the `SendSpawner` it is given) and `reports`, and the panic handler, which resets the microcontroller |
| `README.md` | the board, its state and its memory layout |

`FLASH` in `memory.x` starts behind the factory bootloader and ends before the pages that hold
the serial number and device information, so nothing is ever linked into them. `RAM` ends where
the stack starts: `cortex-m-rt` puts the initial stack pointer at its end, and the host tool and
`cargo xtask check` refuse an image whose stack pointer is anywhere else. The constant
assertion in `src/lib.rs` fails the build when `memory.x` and the board's constants disagree, and
the board's tests check the same. Every board has the same set of files (`cargo xtask check`
compares them).

## A new microcontroller family

Add a PAC (`crates/<chip>-pac`, generated by `cargo xtask pac`) and a chip HAL
(`crates/openwoot-<family>`) with the same file set as the existing chip HALs and
`[package.metadata.openwoot] target`.
