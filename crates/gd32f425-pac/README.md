# gd32f425-pac

Peripheral access crate for the GigaDevice GD32F425 (Wooting 80HE), generated from GigaDevice's SVD by `cargo xtask pac` with svd2rust 0.37.1, form 0.13.0 and nightly rustfmt. `src/`, `build.rs` and `device.x` are generated and committed; change the patches in `xtask/src/pac.rs` and regenerate, never edit the generated files. Regenerating an unchanged input gives byte-identical output.

Features, as svd2rust emits them:

- `rt`: the device vector table for `cortex-m-rt` (16 core + 91 device entries, 107 in total) and the re-exported `#[interrupt]` attribute. Its link section does not compile for every host (macOS rejects it), so enable it for `target_os = "none"` only.
- `critical-section`: `Peripherals::take()`.

## SVD source

`svd/GD32F4xx.svd` is `GD32F4xx.svd` of GigaDevice's device pack `GigaDevice.GD32F4xx_DFP` 3.0.3 (2022-07-27, published on gd32mcu.com), as copied into the `svd/` directory of the [gd32-rs](https://github.com/gd32-rust/gd32-rs) project (commit `50fd1bad899a728eef6130dc86000f88ba307fa0`), which differs from the pack only in whitespace. SHA-256 `400039c2358ae34f9cac08502a11afe2b515ecf7dc47bf3a6f8296fb375a772a`.

## Licence

The SVD file states no licence. GigaDevice distributes it freely with its device packs, and gd32-rs redistributes it in its repository under MIT OR Apache-2.0; this crate does the same. The generated code is distributed under the licence of this repository.

## Patches

Applied to the parsed SVD before generation, so that every register and field the firmware uses agrees with GigaDevice's C peripheral library for the GD32F4xx:

- Access values `read` and `write`, which the SVD format does not define, become `read-only` and `write-only`.
- `FMC`: the page-erase registers of the GD32F425/427/470 are added: `PECFG` at offset `0x20` (`PE_EN` bit 31, `PE_ADDR` bits 0-28) and the write-only `PEKEY` at `0x24` (`PE_KEY` bits 0-31).
- `HS_GLOBAL`: `GSNPSID` at offset `0x40`, the read-only core identification register of the USBHS controller, is added.
- `HS_FIFO`: a new peripheral at `0x4004_1000` with `FIFO[0..6]`, the 32-bit data FIFO windows of the USBHS controller, one per 4 KiB.
