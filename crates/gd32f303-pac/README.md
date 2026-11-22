# gd32f303-pac

Peripheral access crate for the GigaDevice GD32F303 (Wooting 60HE+), generated from GigaDevice's SVD by `cargo xtask pac` with svd2rust 0.37.1, form 0.13.0 and nightly rustfmt. `src/`, `build.rs` and `device.x` are generated and committed; change the patches in `xtask/src/pac/patches.rs` and regenerate, never edit the generated files. Regenerating an unchanged input gives byte-identical output.

Features, as svd2rust emits them:

- `rt`: the device vector table for `cortex-m-rt` (16 core + 60 device entries, 76 in total) and the re-exported `#[interrupt]` attribute. Its link section does not compile for every host (macOS rejects it), so enable it for `target_os = "none"` only.
- `critical-section`: `Peripherals::take()`.

## SVD source

`svd/GD32F30x_XD.svd` is `GD32F30x_XD.svd` (the extra-density GD32F30x parts) of GigaDevice's device pack `GigaDevice.GD32F30x_DFP` 2.2.1 (2022-07-27, published on gd32mcu.com), as copied into the `svd/` directory of the [gd32-rs](https://github.com/gd32-rust/gd32-rs) project (commit `50fd1bad899a728eef6130dc86000f88ba307fa0`), which differs from the pack only by the removed whitespace before the XML declaration. SHA-256 `4ece22501b775725cf93eff5617c621254fc734514c15d6afa4b0ed48e348bc9`.

## Licence

The SVD file states no licence. GigaDevice distributes it freely with its device packs, and gd32-rs redistributes it in its repository under MIT OR Apache-2.0; this crate does the same. The generated code is distributed under the licence of this repository.

## Patches

Applied to the parsed SVD before generation, so that every register and field the firmware uses agrees with GigaDevice's C peripheral library for the GD32F30x:

- Access values `read` and `write`, which the SVD format does not define, become `read-only` and `write-only`.
- `BKP`: every register is 16 bits wide (the SVD says 32), so the PAC issues 16-bit accesses as the library does.
- `USBD`: `EP0CS` to `EP7CS`, `CTL`, `INTF`, `STAT`, `ADDR` and `BADDR` are 16 bits wide (the SVD says 32).
- `FMC`: the register at offset `0x1C` is the option byte status register and is renamed from `OBCTL` to `OBSTAT`.
- `ADC0`, `ADC1` (and `ADC2`, derived from `ADC1`): `CTL0.SYNCM` is 4 bits wide (bits 16-19, the SVD gives 3), and `CTL1.ETEIC` (bit 15, external trigger enable for the inserted channels) is added.
- `USBD_RAM`: a new peripheral at `0x4000_6000` with `DATA[0..256]`, the 512-byte USBD packet buffer as 16-bit half words at a 4-byte stride.
