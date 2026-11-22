# Third-party code

This directory holds the libraries the firmware links. Each one is a copy of an upstream release; only TinyUSB carries local changes. Which files a board compiles follows from its family: the family's `hal.mk` names its peripheral library files (`VENDOR_SRC`) and its TinyUSB device controller driver (`TUSB_PORT_SRC`), see [docs/building.md](../docs/building.md).

| Directory | Project | Version | Upstream | Licence | Local changes |
| --- | --- | --- | --- | --- | --- |
| `littlefs/` | littlefs | v2.4.1 (`LFS_VERSION` 2.4, on-disk format 2.0) | <https://github.com/littlefs-project/littlefs>, tag `v2.4.1` | BSD-3-Clause | none |
| `nanopb/` | nanopb runtime (`pb.h`, `pb_common`, `pb_decode`, `pb_encode`) | 0.4.9.1 | <https://github.com/nanopb/nanopb>, tag `0.4.9.1` | zlib | none |
| `gd32f4xx/` | GigaDevice GD32F4xx Firmware Library, with the CMSIS-Core headers it ships | V3.0.0 (2022-03-09) | GigaDevice, GD32F4xx Firmware Library V3.0.0, <https://www.gd32mcu.com> | BSD-3-Clause (GigaDevice and ARM notices) | none; only part of the library is kept, see below |
| `gd32f30x/` | GigaDevice GD32F30x Firmware Library, with the CMSIS-Core headers it ships | V2.1.3 (the file headers name V2.1.0 as their latest revision) | GigaDevice, GD32F30x Firmware Library V2.1.3, <https://www.gd32mcu.com> | BSD-3-Clause (GigaDevice and ARM notices) | none; only part of the library is kept, see below |
| `tinyusb/` | TinyUSB device stack (core, HID class, Synopsys DWC2 driver, ST full-speed device driver) | commit `f452ab745ef953d74704bc5429ff16a80a5b325a` (2022-05-03, between 0.13.0 and 0.14.0) | <https://github.com/hathach/tinyusb> | MIT | yes, each marked `/* openwoot: ... */`; see below |

`patches/tinyusb-local-changes.patch` is the difference between `tinyusb/` and the upstream commit named above.

## Licence texts

| Directory | Where the licence text is |
| --- | --- |
| `littlefs/` | `littlefs/LICENSE.md` (upstream file); the source files carry an SPDX `BSD-3-Clause` identifier. |
| `nanopb/` | `nanopb/LICENSE.txt` (upstream file). The source files have no licence header. |
| `gd32f4xx/`, `gd32f30x/` | In the header of every source file. `LICENSE.txt` and `cmsis/LICENSE.txt` in each directory are not upstream files: they collect the notices from the file headers and say which file carries which. |
| `tinyusb/` | `tinyusb/LICENSE` (upstream file) and the header of every source file. |

## What is kept of each library

- littlefs: `lfs.c`, `lfs.h`, `lfs_util.c`, `lfs_util.h`.
- nanopb: the C runtime only. The generator is not included; the generated files for the stored settings are in `src/proto/` ([`src/proto/README.md`](../src/proto/README.md)).
- GD32 libraries, each: `peripherals/` (the standard peripheral drivers of the library, upstream directories `Include` and `Source`), `device/` (the device header and the CMSIS system file of the family), `cmsis/` (the four CMSIS-Core headers `core_cm4.h`, `core_cm4_simd.h`, `core_cmFunc.h`, `core_cmInstr.h`) and `gd32f4xx_libopt.h` or `gd32f30x_libopt.h` (from the upstream project template).
- TinyUSB: `src/tusb.*`, `src/common/`, `src/osal/`, `src/device/`, `src/class/hid/`, `src/portable/synopsys/dwc2/` and `src/portable/st/stm32_fsdev/`. Each family compiles one of the two drivers (`TUSB_PORT_SRC` in `hal/<family>/hal.mk`): `gd32f4xx` the DWC2 driver, `gd32f30x` the full-speed device driver.

## Build options

The options are set in the top-level `Makefile`, in `hal/<family>/hal.mk` and in the board's `board.mk`. Third-party sources are compiled with the same processor options and defines as the firmware, but without the project's warning set (`-w`) and without the firmware-only option of `FW_CFLAGS`.

- littlefs: `LFS_NO_MALLOC`, `LFS_NO_ASSERT`, `LFS_NO_DEBUG`, `LFS_NO_WARN`, `LFS_NO_ERROR`. The firmware supplies all buffers statically; see [docs/storage.md](../docs/storage.md).
- nanopb: `PB_NO_ERRMSG`, `PB_WITHOUT_64BIT`. Enums are one byte wide where their values allow it (`-fshort-enums`, applied to the whole build).
- GD32 libraries: the device define comes from the board (`DEVICE_DEFS` in `board.mk`: `GD32F425` for the Wooting 80HE, `GD32F30X_XD` for the Wooting 60HE+). Only the peripheral drivers that the family code calls are compiled (`VENDOR_SRC` in the family's `hal.mk`):
  - `gd32f4xx`: `adc`, `dma`, `fmc`, `gpio`, `i2c`, `pmu`, `rcu`, `spi`, `timer`.
  - `gd32f30x`: `adc`, `bkp`, `dma`, `gpio`, `i2c`, `pmu`, `rcu`, `spi`, `timer`. The flash controller driver (`fmc`) is left out on purpose: the firmware never writes the flash of this family, and without the driver such a call does not link.
  - `device/system_<family>.c` is not compiled. The family's own `SystemInit` in `hal/<family>/system_clock.c` sets up the clocks and points the core at the application's vector table.
  - No startup file of the libraries is used. The vector table and the reset handler are in `hal/<family>/vectors.c` and `hal/<family>/startup.c`.
- TinyUSB: configured by `src/usb/tusb_config.h` (no RTOS (`OPT_OS_NONE`), 6 HID interfaces with 2048-byte endpoint buffers, 64-byte control endpoint, all other device classes off) and by `HAL_DEFS` of the family's `hal.mk`: `gd32f4xx` `CFG_TUSB_MCU = OPT_MCU_STM32F4` with port 1 as a high-speed device; `gd32f30x` `CFG_TUSB_MCU = OPT_MCU_STM32F1` with port 0 as a full-speed device.

## Local changes to TinyUSB

Eight files differ from upstream:

| File | Changes |
| --- | --- |
| `src/portable/st/stm32_fsdev/dcd_stm32_fsdev_pvt_st.h` | Binding for the GD32F303 (`GD32F30X_HD`, `GD32F30X_XD`): includes the GD32 device header and defines the register block, the register bits and the interrupt numbers of its full-speed controller, which match the STM32F1 controller. |
| `src/portable/st/stm32_fsdev/dcd_stm32_fsdev.c` | The driver is enabled for the GD32F303 under `OPT_MCU_STM32F1`. Only the two transfer interrupts are enabled, not the wake-up interrupt. Resume is reported at the first start-of-frame after wake-up. The controller does not enter low-power mode on suspend while the device drives resume signalling. `dcd_connect()` and `dcd_disconnect()` are supplied by the family code (`hal/gd32f30x/hal_usb.c`). |
| `src/portable/synopsys/dwc2/dwc2_stm32.h` | The STM32F4 binding includes the GD32 device header and defines the core addresses, endpoint counts and FIFO sizes of the GD32F425. The interrupt is selected by port number at run time. The external ULPI PHY is selected explicitly. |
| `src/portable/synopsys/dwc2/dcd_dwc2.c` | The high-speed core with an external 8-bit ULPI PHY is hard-coded, because the GD32 core does not provide usable hardware configuration registers. The application can force full speed (`g_usb_high_speed_enabled`, defined in `hal/gd32f4xx/hal_usb.c`). FIFOs are flushed after the core reset, at bus reset and when an IN endpoint is opened. Changed order and acknowledgement of endpoint interrupts. A bus reset is reported with an invalid speed until enumeration is done. Resume is reported at the first start-of-frame after wake-up. |
| `src/device/usbd.c`, `src/device/usbd.h` | Repeated bus resets and unplug events on an idle bus are dropped. Device state is cleared in task context instead of in the interrupt. `SET_CONFIGURATION(0)` calls `tud_umount_cb()`. The stack records how the host asks for string descriptors (`tud_os_detect_get()`). |
| `src/class/hid/hid_device.c`, `src/class/hid/hid_device.h` | Report length is 16 bit. Feature reports from `SET_REPORT` use a separate 64-byte buffer. `tud_hid_set_report_cb()` returns `bool`; when it returns `false` for a report from the OUT endpoint, the endpoint stays idle until the application calls `tud_hid_n_receive_rearm()`. New functions `tud_hid_itf_num_to_instance()` and `tud_hid_n_receive_rearm()`, new callbacks `tud_hid_report_failed_cb()` and `tud_hid_os_detected_cb()`. |

The comment at each change says what it does and why. The changes to the full-speed device driver have never run on hardware.

## Updating a library

littlefs, nanopb, GD32 libraries:

1. Replace the files listed under "What is kept of each library" with the files of the new release. Keep the upstream licence file. For a GD32 library, check that the notices collected in its `LICENSE.txt` and `cmsis/LICENSE.txt` still match the file headers.
2. Update the version in the table above.
3. Build every board (`make all-boards`), run `make check`, and test on a keyboard. A new littlefs or nanopb version must still read the data an older build wrote: the file system in the settings flash is on-disk format 2.0, and the settings are protobuf messages described in `src/proto/`.

TinyUSB:

1. Replace the files under `tinyusb/src/` with the same files of the new upstream commit, and `tinyusb/LICENSE` with the upstream one.
2. Apply the local changes, from the directory `third_party/tinyusb`:

   ```
   patch -p1 < ../patches/tinyusb-local-changes.patch
   ```

   Resolve rejected hunks by hand. Every change keeps its `/* openwoot: ... */` comment.
3. Regenerate the patch. With the unmodified upstream files in a directory `a/` and a copy of `third_party/tinyusb` in a directory `b/` next to it:

   ```
   diff -ruN a b > tinyusb-local-changes.patch
   ```

   Keep the header lines at the top of the patch file and update the commit they name.
4. Update the commit in the table above, then build every board, run `make check` and test on a keyboard.
