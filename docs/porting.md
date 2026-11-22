# Porting

This document describes how to extend the tree: a keyboard whose microcontroller family is already supported, a new microcontroller family, a new chip driver, a new feature and a new strategy. For each it names the files, the build variables, and what `make check` enforces. It ends with a checklist and with what has run on which hardware.

Read [architecture](architecture.md) first: it explains the layers, the HAL interface, drivers, features, strategies and registration. [Building](building.md) explains how the build turns a board directory into a source list.

Related documents: [hardware](hardware.md) and [layout](layout.md) (the level of detail a board description needs), [bootloader](bootloader.md) and [flashing](flashing.md) (how an image gets onto a keyboard), [storage](storage.md) and [protocol](protocol.md) (the formats a port must keep), [`tools/README.md`](../tools/README.md) (the host tool and `board.json`).

## What goes where

Nothing in the tree lists the boards, families, drivers, features or strategies. Each is found by its directory, and adding one adds files only:

| Adding | New files | Other files touched |
| --- | --- | --- |
| a keyboard of a supported family | `boards/<board>/` | none |
| a microcontroller family | `hal/<family>/`, its peripheral library in `third_party/<library>/` | possibly a TinyUSB port driver in `third_party/tinyusb/` |
| a chip driver of an existing class | `drivers/<chip>/` | the `board.mk` of the boards that use it |
| a driver class | `drivers/include/drivers/<class>.h`, `drivers/<chip>/` | the `board.mk` of **every** board (each must list a driver of every class), the module that uses it |
| a feature | `src/<module>/<feature>/` | callers in `src/` (unconditional calls into the feature's header), `board.mk` and a `board.h` block of the boards that list it |
| a strategy slot | `src/<module>/<slot>/` with one directory per variant | the module that calls it, the `board.mk` of **every** board (`STRATEGY_<slot>`) |
| a strategy variant | `src/<module>/<slot>/<variant>/` | the `board.mk` of the boards that select it |

Where the boards differ, the kind of difference decides the place:

| Difference | Place |
| --- | --- |
| a number (dimension, address, count, gain, USB id) | a constant in `board.h` |
| a pin | a constant in `wiring.h`, read by the family code only |
| data (key maps, LED maps, factory colours and profiles) | `boards/<board>/tables/`, `boards/<board>/defaults/` |
| an optional capability (light bar, measured reference) | a feature |
| another algorithm for the same job | a strategy variant, named for what it does |
| another microcontroller | a family |
| another chip on the board | a driver |

Code never goes into a board directory, and nothing in `src/`, `drivers/` or `hal/include/` tests a board or family name.

## A board

A board directory has exactly this file set (`board-files` rule):

| File | Content |
| --- | --- |
| `board.mk` | What the firmware of the board consists of (below) |
| `board.h` | The board in numbers as `BOARD_` constants (below) |
| `wiring.h` | The pins: the constants that `hal/<family>/wiring_template.h` lists, in that order (`board-wiring` rule) |
| `board.json` | What the host tool needs (below) |
| `linker.ld` | The memory regions `FLASH` and `RAM`, then `INCLUDE sections.ld` |
| `README.md` | What the directory contains and the facts of the board |
| `tables/adc_scan_tables.c`, `.h` | `k_adc_scan_parallel_mux_order`: the multiplexer read by each result of a parallel conversion |
| `tables/layout_tables.c`, `.h` | Factory key map layers (`k_layout_default_keymap_layers`, `BOARD_DEFAULT_KEYMAP_LAYERS` entries) and per layout variant the maps between scan positions and key ids, the key-to-LED map, the space-bar fill columns and the key codes the variant writes into layer 0; collected in `k_layout_variants` (`layout_variant_t`, [`src/layout/layout_types.h`](../src/layout/layout_types.h)) |
| `tables/rgb_tables.c`, `.h` | Key-to-LED maps per variant; `k_rgb_led_channels` (LED number → frame page and channel) and `k_rgb_frame_pages` (frame page → register bank and run of the driver), element types in [`src/rgb/rgb_frame_layout.h`](../src/rgb/rgb_frame_layout.h) |
| `tables/profile_tables.c`, `.h` | Factory colour maps (RGB565) of the default profiles and layers |
| `defaults/profile_defaults.c` | The factory profiles as data (`profile_default_set_t`) and the four functions of [`src/profile/profile_defaults.h`](../src/profile/profile_defaults.h): `profile_fill_default_rgb_settings`, `profile_copy_default_base_colors`, `profile_fill_default_cfg`, `profile_create_factory_defaults`; with feature `edition` also `k_profile_edition_default_set` |
| `layout/key_positions_<variant>.csv` | Position, size, LED number and default key code of every key; one file per layout variant (`board-layout` rule). Reference data, not compiled. |
| `layout/leds.csv` | Position of every LED. Reference data, not compiled. |

The two existing boards, [`boards/wooting_80he/`](../boards/wooting_80he/) and [`boards/wooting_60he_plus/`](../boards/wooting_60he_plus/), are the reference; their `README.md` files describe every file.

### `board.mk`

Every name in `board.mk` is a directory; the `Makefile` stops with a message that lists the valid names when one does not exist.

| Variable | Meaning | Example (`wooting_80he`) |
| --- | --- | --- |
| `FAMILY` | The directory `hal/<family>/` | `gd32f4xx` |
| `DEVICE_DEFS` | The device define of the family's peripheral library | `-DGD32F425` |
| `DRIVERS` | One driver of every class, directories `drivers/<chip>/` | `is31fl3741 spi_nor` |
| `FEATURES` | The features the board has, directory names `src/<module>/<feature>/`; a feature's `FEATURE_REQUIRES` must be listed too | `signals lightbar edition recheck` |
| `STRATEGY_<slot>` | One variant for every strategy slot | `STRATEGY_noise_recal := minmax` |

### `board.h`

`board.h` is found through the include path of the board directory, so `#include "board.h"` always means the board being built. Every board defines the same constants in the same order (`board-constants` rule); a constant that does not apply to a board belongs to a feature, and then it is in a block `/* ---- Feature: <name> ---- */` at the end of the file, present only on boards that list that feature. Dimensions are plain `int` constants; a module that needs an unsigned constant converts the value in its own header.

| Group | Constants |
| --- | --- |
| Identity | `BOARD_USB_VID`, `BOARD_USB_PID_BASE` (the firmware adds the USB mode 0..2), `BOARD_USB_MANUFACTURER`, `BOARD_USB_PRODUCT` |
| Image | `BOARD_APP_BASE`, `BOARD_FLASH_END`, `BOARD_RAM_END`; `linker.ld` and `board.json` repeat them (`board-json` rule) |
| Key matrix | `BOARD_KEY_ROWS`, `BOARD_KEY_COLS`, `BOARD_KEYMAP_COLS`, `BOARD_KEY_POSITIONS`, `BOARD_KEY_FIRST_ROW` |
| Scan | `BOARD_MUX_COUNT`, `BOARD_MUX_ADDR_LINES`, `BOARD_MUX_POSITIONS`, `BOARD_MUX_ADDRESSES_USED`, `BOARD_SCAN_POSITIONS` |
| Converter | `BOARD_ADC_ZERO_LEVEL`, `BOARD_SCAN_OVERSAMPLE`, `BOARD_SCAN_SAMPLE_HALF_CYCLES` |
| Layout | `BOARD_LAYOUT_VARIANT_COUNT`, `BOARD_DEFAULT_KEYMAP_LAYERS` |
| Lighting | `BOARD_LED_LIMIT`, `BOARD_LED_NONE`, `BOARD_RGB_FRAME_PAGE_COUNT`, `BOARD_RGB_FRAME_PAGE_PWM_MAX`, `BOARD_LED_GAIN_RED`, `BOARD_LED_GAIN_GREEN`, `BOARD_LED_GAIN_BLUE`, `BOARD_RGB_LEVEL_METER_SEGMENTS` |
| LED driver | `BOARD_LED_DRIVER_ADDRESS`, `BOARD_LED_DRIVER_CONFIG_RUN` |
| Storage | `BOARD_STORAGE_LFS_BLOCK_COUNT`, `BOARD_DEVICE_INFO_ADDR` (address in the medium of the `device_info` variant), `BOARD_MCU_SERIAL_PAGE_ADDR` |
| Feature `lightbar` | `BOARD_LIGHTBAR_LED_COUNT`, `BOARD_LIGHTBAR_LEDS` |
| Feature `edition` | `BOARD_EDITION_SERIAL_CODES`, `BOARD_EDITION_USB_PRODUCT`, `BOARD_EDITION_FACTORY_PROFILE_COUNT`, `BOARD_EDITION_DEFAULT_SWITCH_TYPE` |
| Feature `recheck` | `BOARD_SCAN_RECHECK_FIRST_INPUT` |

The product name of a board may appear in `board.h` strings, `board.json` and the board's `README.md`, nowhere in the shared code (`display-names` rule).

### `board.json`

Read by [`tools/openwoot.py`](../tools/openwoot.py), which has no list of keyboards of its own:

| Key | Content |
| --- | --- |
| `name` | The name the board is sold under |
| `usb.vendor_id`, `usb.product_id_base` | As `BOARD_USB_VID`, `BOARD_USB_PID_BASE` |
| `image.base`, `image.end`, `image.stack_top` | As `BOARD_APP_BASE`, `BOARD_FLASH_END`, `BOARD_RAM_END` |
| `image.vectors` | Number of entries of the family's `g_vector_table` |
| `update.supported` | `true` only when the board's update mode is known. Then `update.product_id` (the bootloader's product id in update mode) and `update.block_size` follow; otherwise `update.reason` says why, and the host tool refuses `enter` and `flash` for the board. |
| `build` (optional) | `"bring-up"`: the board does not link yet. `make BOARD=<board>` then compiles every layer and links no image, and `make check` leaves the board out of the image checks. |

`make check` compares every value with `board.h`, `linker.ld` and the vector table.

### Bring-up of a board

1. Collect the facts: microcontroller, clock source, where the application starts and what the bootloader expects of an image, key matrix and sensor multiplexing, LED driver and LED wiring, settings flash, USB ids of the firmware and of the bootloader. [Hardware](hardware.md) shows the level of detail needed.
2. Copy the board of the same family and rename it: `cp -R boards/wooting_80he boards/<board>`. Add `"build": "bring-up"` to its `board.json` until the tables are complete.
3. Fill in `board.mk`, `board.h`, `wiring.h`, `linker.ld`, `board.json`. `make BOARD=<board>` compiles each layer; `make BOARD=<board> hal-objects` (and `driver-objects`, `board-objects`, `src-objects`, `third-party-objects`) compiles one layer alone.
4. Fill in the tables in the order the firmware needs them: the scan tables and the scan position ↔ key id maps (without them no key is read; the two maps must be inverse to each other), the LED channel and frame page tables, the key-to-LED maps, then the factory key maps, colours and profiles.
5. Remove the `build` entry; `make BOARD=<board>` now links. `make check` must pass.
6. Write `layout/` and `README.md`.
7. Before the first write to a keyboard: the board's update mode and its recovery path must be known and described ([bootloader](bootloader.md) and [flashing](flashing.md) do this for the Wooting 80HE). Until then `update.supported` stays `false`. `python3 tools/openwoot.py plan build/<board>/openwoot-<board>.hex` checks an image against `board.json` without talking to a keyboard.

## A microcontroller family

A family directory `hal/<family>/` has the same file names as every other family (`hal-files` rule):

| File | Content |
| --- | --- |
| `hal.mk` | `ARCH` (processor options), `HAL_DEFS` (TinyUSB binding: `CFG_TUSB_MCU` and the root port mode), `VENDOR_INCS` and `VENDOR_SRC` (the peripheral library: include paths and the source files the family calls), `TUSB_PORT_SRC` (the TinyUSB device controller driver). The `Makefile` stops if `ARCH`, `HAL_DEFS`, `VENDOR_INCS` or `TUSB_PORT_SRC` is empty. |
| `hal_led_bus.c`, `hal_mcu.c`, `hal_nor_bus.c`, `hal_scan.c`, `hal_system.c`, `hal_time.c`, `hal_usb.c` | One implementation per header of [`hal/include/hal/`](../hal/include/hal/) (`hal-area`, `hal-functions` rules) |
| `inline/hal_irq_inline.h`, `inline/hal_scan_inline.h`, `inline/hal_time_inline.h` | The inline functions of `hal_irq.h`, `hal_scan.h`, `hal_time.h`. They may include only `<stdint.h>`, `<stdbool.h>` and the board's `wiring.h` (`hal-inline` rule), because firmware and drivers include them; a register they need is written as an address in the header and checked against the library in the family's `.c` file. |
| `startup.c` | `Reset_Handler`, `Default_Handler` |
| `system_clock.c` | `SystemInit` and `SystemCoreClock`; the library's own `system_<family>.c` is not compiled |
| `vectors.c` | `g_vector_table` in section `.isr_vector`: the tick vector → `hal_time_tick_isr`, the conversion interrupt → `hal_scan_conversion_isr`, the USB controller → the family's handler that calls TinyUSB; every other vector → `Default_Handler` |
| `sections.ld` | The section layout; must contain `KEEP(*(SORT(.registry.*)))` (`hal-registry` rule) and define `_estack`, `_sidata`, `_sdata`, `_edata`, `_sbss`, `_ebss` |
| `wiring_template.h` | Not compiled: the list of pin constants a board of this family defines in its `wiring.h` |

What the interface asks is described in its headers; read them all before writing a family. Points that are easy to miss:

- `hal_irq_save` must return a state that `hal_irq_restore` understands, and both must be callable nested.
- `hal_time_micros` is read in the conversion interrupt; only its low 16 bits must be exact in every context. The family's `inline/hal_time_inline.h` documents where its counter wraps.
- `hal_scan_init` waits through the firmware's call-back `hal_scan_wait_ms`; the family has no clock of its own.
- The register write order of the scan, LED bus and NOR bus code is part of the behaviour for a board whose stock firmware has the same order. Keep it where a board of the family depends on it.
- `hal_mcu_flash_write_page` returns `false` on a family whose firmware must not write its own flash; then the peripheral library's flash driver need not be in `VENDOR_SRC` at all, so that a call cannot link (as on `gd32f30x`).
- The USB controller needs a TinyUSB port driver and binding. Local changes to TinyUSB are listed in [`third_party/README.md`](../third_party/README.md) and must be added to `third_party/patches/tinyusb-local-changes.patch`.

The peripheral library goes to `third_party/<library>/` with its licence notices; `third_party/README.md` lists it.

## A chip driver

A driver is a directory `drivers/<chip>/` with:

- `driver.mk`: a comment that says what the chip is and which HAL headers the driver uses, and `DRIVER_CLASS := <class>`;
- the driver sources, which include only their own headers, `drivers/<class>.h`, `hal/*.h`, `board.h` and TinyUSB (`tusb.h`, for `tud_task` in blocking waits). Board-specific settings of the chip are `board.h` constants.

The class header `drivers/include/drivers/<class>.h` must exist (`driver-class` rule). A new class is a new header there; because the build requires exactly one driver of every class on every board, a new class means every `board.mk` lists a driver for it. Two chips of one class on one board (two LED drivers) are not provided for by the current classes.

## A feature

A feature is a directory `src/<module>/<feature>/` inside the module it extends. The feature name must be unique across modules (the build addresses features by directory name).

1. `feature.mk`: a comment that says what the feature is and names its public header; `FEATURE_REQUIRES := <feature> ...` if it needs another feature.
2. The public header `<feature>.h`, the only file where `FEATURE_<NAME>` appears (`feature-define` rule):

   ```c
   #if FEATURE_EXAMPLE
   void example_step(void);
   #else
   static inline void example_step(void)
   {
   }
   #endif /* FEATURE_EXAMPLE */
   ```

   Callers include the header and call the function on every board. A function that returns a value returns the neutral value without the feature (see [`rgb/signals/signals.h`](../src/rgb/signals/signals.h), [`profile/edition/edition.h`](../src/profile/edition/edition.h)). A function on the per-sample path of the scan is inline also with the feature (see [`adc_scan/recheck/recheck.h`](../src/adc_scan/recheck/recheck.h), [`adc_scan/reference/reference.h`](../src/adc_scan/reference/reference.h)).
3. The sources, compiled only when a board lists the feature. Everything that belongs to the feature lives here: its state, its host commands (`PROTOCOL_FEATURE_COMMAND`, `PROTOCOL_REPORT_COMMAND`), its profile sections (`PROFILE_SECTION`), its start-up steps (`STARTUP_STEP`). Order numbers are placed relative to a core slot in the feature's own files, for example `STARTUP_SIGNALS_KEY_RATE = STARTUP_KEYS_TASK + 1` in [`rgb/signals/signals_key_rate.c`](../src/rgb/signals/signals_key_rate.c).
4. Board constants of the feature go into a block `/* ---- Feature: <feature> ---- */` at the end of `board.h` of the boards that list it.

Without the feature, a command number it would register is answered as unsupported by the dispatcher, a profile section or start-up step simply does not exist.

## A strategy

A strategy slot is a directory `src/<module>/<slot>/` with:

1. `strategy.mk`: a comment that describes the slot, names its interface header and lists the files every variant has;
2. the interface header `<slot>.h`: the functions every variant implements;
3. one sub-directory per variant, each with the same file names (`strategy-variants` rule), named for what the variant does.

The build compiles the `.c` files of the selected variant and puts its directory on the include path, so a header with the same name in every variant (as `noise_stats.h` in [`src/analog/noise_recal/`](../src/analog/noise_recal/)) is the selected variant's. Every board must select a variant of every slot (`STRATEGY_<slot>` in `board.mk`); the `Makefile` stops with the list of variants otherwise. A shared part that every variant uses stays in the module (as the device information format in [`src/storage/storage_device_info.c`](../src/storage/storage_device_info.c)).

## What `make check` enforces

`make check` runs `tools/check_tree.py` on the tree, then builds the image of every board that links with `LTO=0` and with `LTO=1` in a temporary directory and checks their registration lists. `python3 tools/check_tree.py --list-rules` prints every rule; [architecture](architecture.md#layers) explains them. The `Makefile` itself stops on an unknown board, family, driver, feature or variant, a missing required feature, a missing or duplicate driver class, a missing strategy selection and an incomplete `hal.mk`.

## Checklist

For a new board:

- [ ] `boards/<board>/` has the complete file set; `make boards` lists it.
- [ ] `board.mk` names an existing family, its device define, one driver per class, the features, a variant for every slot.
- [ ] `board.h` has every constant in the order of the other boards, and a block per listed feature that needs constants.
- [ ] `wiring.h` has every constant of the family's `wiring_template.h`, in order.
- [ ] `linker.ld`, `board.h` and `board.json` agree on base, end and stack top; `image.vectors` is the family's vector count.
- [ ] The two maps between scan positions and key ids are inverse to each other for every variant; the LED tables cover every LED number below `BOARD_LED_LIMIT`.
- [ ] One `layout/key_positions_<variant>.csv` per layout variant; `leds.csv`.
- [ ] `make BOARD=<board>` links for `LTO=0` and `LTO=1`; `make check` passes.
- [ ] `update.supported` is `true` only if the update mode and the recovery path are known.
- [ ] `README.md` states what has and has not run on a device.

For a family, a driver, a feature or a strategy: every existing board still builds (`make all-boards`) and `make check` passes.

## What has run where

What is shared in this tree has been shaped by two boards, but only one of them has ever run this code, and not in its current form:

| Part | State |
| --- | --- |
| Wooting 80HE (`gd32f4xx`, `is31fl3741`, `spi_nor`, features `signals`, `lightbar`, `edition`, `recheck`, variants `minmax`, `nor_flash_sector`, `device_info`) | An earlier form of this code, before it was divided into layers, HAL, drivers, features and strategies, ran on one keyboard (ANSI layout): typing, lighting, profiles, rapid trigger. The restructured tree has not yet been run on hardware. |
| Wooting 60HE+ (`gd32f30x`, feature `reference`, variants `stddev`, `mcu_flash_page_readonly`, `mcu_option`) | Builds; its behaviour was compared with stock firmware 2.14.1 of this board off the device. Never run on a keyboard. Its update mode has not been examined, and the host tool refuses to flash it. |

Consequences for a port:

- The HAL interface has been implemented twice, but only the `gd32f4xx` implementation of its predecessor has run. Timing assumptions of the scan (trigger delays, settling times in `src/adc_scan/adc_scan_types.h`) are those of the two existing boards.
- The shared tables in `src/tables/` assume the switches, sensors and USB interfaces of these boards: the travel curves of the known switch types, the USB descriptors in [`src/tables/usb_tables.c`](../src/tables/usb_tables.c), the firmware version reported to the host (`PROTOCOL_VERSION_MAJOR` and following, [`src/protocol/protocol_device.c`](../src/protocol/protocol_device.c)).
- A key id is one byte, `row << 5 | column` (`KEYS_KEY_ID` in [`src/keys/keys_types.h`](../src/keys/keys_types.h)): at most 32 columns, and `KEYS_KEY_ID_MAX` allows rows 0 to 5 only.
- The settings schema has fixed matrix sizes: 6 rows and 21 columns in [`src/proto/settings.options`](../src/proto/settings.options) (`KeyMatrixU8.rows`, `KeymapLayer.rows`, `KeyMatrixU16Row.values` and others). A board whose stock firmware uses other sizes needs another schema, and the vendor's configuration program decides whether that works.
- The sensor scan implements one scheme: analog multiplexers with shared address lines, one converter input each, a timer-triggered conversion, single-input and parallel scanning. A board that reads its sensors otherwise needs a new `adc_scan` design, not only a family.
- The update procedure of the host tool is that of the factory bootloader of the Wooting 80HE.
