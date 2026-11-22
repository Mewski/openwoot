# Persistent storage

This document describes everything the keyboard keeps across a power cycle: where it lives in flash, the file system and its parameters, every file with its format, how files are written and what follows from that, what happens on the first boot and on a factory reset, and the Protocol Buffers schema of the stored messages.

The formats match stock firmware 2.14.1: openwoot mounts the file system the stock firmware left on the keyboard and uses the settings in it, and the stock firmware does the same after it is flashed back.

The document covers the boards of this repository, the Wooting 80HE and the Wooting 60HE+. The file system, the file formats and the record formats are the same on both; where the data lies, which files exist and some file sizes depend on the board and are collected in [Differences between the boards](#differences-between-the-boards). openwoot has never run on a Wooting 60HE+: what this document states for that board is taken from its stock firmware, not confirmed on hardware.

Related documents: [protocol](protocol.md) (the host commands that read and write these files), [architecture](architecture.md) (boot sequence and main loop), [hardware](hardware.md) (the SPI flash chip, its bus and the flash layout of each board), [flashing](flashing.md) (the MCU's internal flash of the 80HE), [bootloader](bootloader.md) (the 80HE's factory bootloader, which reads the serial page and can request a settings reset).

## Where data lives

Every board keeps its data in three places: a littlefs file system on an external SPI NOR flash chip (profiles and settings), a device-info record with factory data outside the file system, and a copy of the serial number text in a page of the MCU's own flash. The addresses are constants of the board's `board.h` ([80HE](../boards/wooting_80he/board.h), [60HE+](../boards/wooting_60he_plus/board.h)).

| Content | Wooting 80HE | Wooting 60HE+ |
| --- | --- | --- |
| littlefs file system: profiles and settings (`BOARD_STORAGE_LFS_BLOCK_COUNT` blocks of 4096 bytes from offset 0) | SPI flash `0x000000`-`0x7FEFFF`, 2047 blocks | SPI flash `0x000000`-`0x7FFFFF`, 2048 blocks: the whole chip |
| device-info record, factory data (`BOARD_DEVICE_INFO_ADDR`) | SPI flash `0x7FF000`-`0x7FFFFF`, the last 4096-byte sector; read and written | MCU flash `0x080FF000`-`0x080FFFFF`; read only |
| copy of the serial number text (`BOARD_MCU_SERIAL_PAGE_ADDR`) | MCU flash `0x080FE000`-`0x080FEFFF`, one 4096-byte page | MCU flash page at `0x080FD000`; openwoot never writes it |

The external chip is an 8 MiB SPI NOR flash with 24-bit addresses on both boards. The self-test command accepts manufacturer id `0xEF` (Winbond) or `0xC8` (GigaDevice) with device id `0x16`, which matches 64 Mbit parts such as W25Q64 and GD25Q64. The driver is [`spi_nor.c`](../drivers/spi_nor/spi_nor.c) (interface [`nor_flash.h`](../drivers/include/drivers/nor_flash.h)); it sends its commands through the bus transfers of the family's `hal_nor_bus.c` ([gd32f4xx](../hal/gd32f4xx/hal_nor_bus.c) for the 80HE, [gd32f30x](../hal/gd32f30x/hal_nor_bus.c) for the 60HE+). The bus and pins of each board are described in [hardware](hardware.md#spi-nor-flash-memory).

### The device-info record

One record holds the factory data of the unit. It is not a file, so formatting the file system does not touch it. Its format is the same on every board (constants in [`device_info.h`](../src/storage/device_info/device_info.h), decoding in [`storage_device_info.c`](../src/storage/storage_device_info.c)). Where it lies and whether it can be written is the strategy slot `device_info`: the board's `board.mk` selects a variant with `STRATEGY_device_info`, and the variant's `device_info.c` implements `storage_read_device_info` and `storage_write_device_info`.

| Board | Variant | Medium |
| --- | --- | --- |
| Wooting 80HE | [`nor_flash_sector`](../src/storage/device_info/nor_flash_sector/device_info.c) | the last sector of the SPI flash, `0x7FF000`; read and written |
| Wooting 60HE+ | [`mcu_flash_page_readonly`](../src/storage/device_info/mcu_flash_page_readonly/device_info.c) | the MCU flash page at `0x080FF000`; read with `hal_mcu_flash_read`, never written |

The record format:

| Address | Content |
| --- | --- |
| `BOARD_DEVICE_INFO_ADDR` | length prefix, then one encoded [`DeviceInfo`][m-devinfo] message |
| `BOARD_DEVICE_INFO_ADDR` + `0xFFC` (`0x7FFFFC` on the 80HE, `0x080FFFFC` on the 60HE+) | 16-bit magic, little endian, that tells the format of the length prefix |

| Magic | Constant | Length prefix | Largest payload |
| --- | --- | --- | --- |
| `0x1337` | `STORAGE_DEVICE_INFO_MAGIC_LEN8` | 1 byte | 254 bytes |
| `0x1338` | `STORAGE_DEVICE_INFO_MAGIC_LEN16` | 2 bytes, little endian | 4090 bytes (`0xFFA`) |

Reading: if the magic is neither value, the reader tests the first 30 bytes of the record. All `0xFF` means "blank" (`STORAGE_ERR_BLANK`, -2). Anything else is treated as the 1-byte format on the 80HE. On the 60HE+ it is read as an older raw record, as that board's stock firmware does: the fields of `SerialNumber` without the three optional ones, then a length byte and an encoded `DeviceInfoAux`; such a record carries no physical layout and no lock mode. A zero or over-long length, or a protobuf decode failure, returns `STORAGE_ERR_DECODE` (-3).

Writing (80HE): the message is encoded, the sector is erased, the length and payload are programmed, and the magic is programmed last. The writer uses the 1-byte format when the payload is at most 254 bytes, otherwise the 2-byte format. Nothing is verified afterwards. A power loss between the erase and the end of programming loses the record, including the serial number; there is no second copy of the full record.

Writing (60HE+): `storage_write_device_info` stores nothing and returns `STORAGE_ERR_READ_ONLY` (-5). The variant contains no code that erases or programs flash, so the factory record cannot be lost through openwoot. The stock firmware 2.14.1 of that board rewrites the page.

Content of `DeviceInfo`:

| Field | Meaning | Used by |
| --- | --- | --- |
| `serial_number` ([`SerialNumber`][m-serial]) | Factory serial number. A first field of `0xFFFF` means "not programmed". | `storage_format_serial_string` builds the USB serial string from it; `profile_edition_is_special` (feature `edition`) reads the edition |
| `f2` ([`DeviceInfoAux`][m-aux]) | A record the firmware only stores and returns; it does not interpret the content. | [protocol](protocol.md) feature command 59, OUT-report command 28 |
| `physical_layout` | 0 ANSI, 1 ISO, 2 JIS | on the 80HE the physical layout (see [Physical layout](#physical-layout)); byte 5 of the device settings block. The 60HE+ does not use this field. |
| `lock_mode` | 1 and 3 lock the switch profiles and host signals; the firmware treats both values the same. A set lock cannot be removed through the protocol, see [protocol](protocol.md#lock-mode). | `device_info_load_lock_mode` ([`storage_init.c`](../src/storage/storage_init.c)) copies it to `g_profile_lock_mode` at boot |

On the 80HE the record is rewritten by OUT-report commands 7 (device settings, because the physical layout is stored here), 12 (serial number), 28 and 39 (lock mode), and by the boot code ([`settings.c`](../src/app/settings.c)) when it corrects a stored USB mode. On the 60HE+ none of them changes it.

`storage_format_serial_string` ([`storage_serial.c`](../src/storage/storage_serial.c)) prints the serial number as text in this order (optional parts in brackets):

```
A<f1>B<f2><f3>W<f4>[T<f10>]<f5>[<f11>][S<edition>]<stage letter><f6>
```

`f1`..`f4`, `f10`, `f11` and `edition` are printed with two digits, `f5` with one, `f6` with five. The stage letter is `P`, `T`, `E` or `X` for `stage` 1..4 and `H` for every other value. This text is the USB serial string of the keyboard and the content of the serial page described below.

### Physical layout

The physical layout of the unit is an index of the board's `k_layout_variants` (in `tables/layout_tables.c` of the board directory); it selects the layout tables at boot and is byte 5 of the device settings block. Where it is kept is the strategy slot `layout_variant` ([`layout_variant.h`](../src/storage/layout_variant/layout_variant.h)), selected by `STRATEGY_layout_variant` in `board.mk`; the variant implements `storage_layout_variant_load` and `storage_layout_variant_store`.

| Board | Variant | Layouts | Stored |
| --- | --- | --- | --- |
| Wooting 80HE | [`device_info`](../src/storage/layout_variant/device_info/layout_variant.c) | 0 ANSI, 1 ISO, 2 JIS | field `physical_layout` of the device-info record; `storage_save_device_settings` rewrites the record after it wrote `ds` |
| Wooting 60HE+ | [`mcu_option`](../src/storage/layout_variant/mcu_option/layout_variant.c) | 0 ANSI, 1 ISO | bit 0 of the MCU's user option byte (`hal_mcu_read_user_option`); the firmware never writes it, so storing does nothing. Matches stock firmware 2.14.1. |

### Edition

Feature `edition` ([`src/profile/edition/`](../src/profile/edition/)) distinguishes a regular unit from a unit of a special edition by the optional field `edition` of the serial number. Only the 80HE lists the feature: the values 9 and 99 (`BOARD_EDITION_SERIAL_CODES`) select its TenZ edition (`profile_edition_is_special` in [`edition.c`](../src/profile/edition/edition.c)). The edition decides the USB product string (`BOARD_EDITION_USB_PRODUCT`), the number of factory profiles (`BOARD_EDITION_FACTORY_PROFILE_COUNT`, 3), the factory default profiles (`k_profile_edition_default_set`) and the default switch type (`BOARD_EDITION_DEFAULT_SWITCH_TYPE`, 1). If the device-info record cannot be read the unit counts as regular. On the 60HE+ every unit is a regular one.

### Serial mirror in MCU flash

`storage_mirror_serial_to_mcu_flash` ([`storage_serial.c`](../src/storage/storage_serial.c)) writes the serial number text into the MCU's own flash at `BOARD_MCU_SERIAL_PAGE_ADDR` as one length byte followed by the ASCII text, through `hal_mcu_flash_write_page`. It runs at boot if the first byte of the page is `0xFF` (`device_serial_mirror_init`), and after the host programmed a new serial number.

- 80HE: the page is `0x080FE000`. The family's `hal_mcu_flash_write_page` ([`hal_mcu.c`](../hal/gd32f4xx/hal_mcu.c)) erases the 4 KiB page and programs the record in 16-bit steps.
- 60HE+: the page is `0x080FD000`. The family's `hal_mcu_flash_write_page` ([`hal_mcu.c`](../hal/gd32f30x/hal_mcu.c)) does nothing: openwoot never erases or programs the flash of this MCU, so the page keeps what the factory put there.

The firmware never reads the text back. On the 80HE the page exists for the factory bootloader (version 2.3), which occupies `0x08000000`-`0x08007FFF` and has no access to the SPI flash: at its start it reads the length byte and the text from `0x080FE000`, uses the text as its USB serial string in update mode and returns it as the answer to its serial command. The bootloader never writes the page. If the page is erased, the bootloader derives a substitute string from the MCU's unique id, and the firmware writes the page again at its next boot. [Bootloader](bootloader.md) describes update mode; [flashing](flashing.md) covers the rest of the internal flash layout. What the 60HE+ bootloader does with its page is not known.

## The file system

The file system is littlefs, a small fail-safe file system for microcontrollers, in version 2.4, which writes on-disk format 2.0. The sources are in `third_party/littlefs/` ([`lfs.c`](../third_party/littlefs/lfs.c)).

### Parameters

One instance exists: `g_storage_lfs`, configured by `g_storage_lfs_config` ([`storage_data.c`](../src/storage/storage_data.c)).

| Parameter | Value | Meaning |
| --- | --- | --- |
| `read_size`, `prog_size` | 1, 1 | the flash can be read and programmed byte by byte |
| `block_size` | 4096 | one erase sector of the flash |
| `block_count` | `BOARD_STORAGE_LFS_BLOCK_COUNT`: 2047 (`0x7FF`) on the 80HE, 2048 (`0x800`) on the 60HE+ | 80HE: `0x7FF000` bytes, the last sector of the chip is left to the device-info record; 60HE+: the whole chip |
| `block_cycles` | 500 | erase cycles after which littlefs moves a metadata block (wear levelling) |
| `cache_size` | 128 | size of the read cache, the program cache and every per-file cache |
| `lookahead_size` | 64 bytes | the block allocator looks at 512 blocks at a time |
| `name_max`, `file_max`, `attr_max`, `metadata_max` | 0 | littlefs defaults: 255 characters, `0x7FFFFFFF` bytes, 1022 bytes, one block |
| buffers | static | `g_storage_lfs_read_buffer`, `g_storage_lfs_prog_buffer` (128 bytes each), `g_storage_lfs_lookahead_buffer` (64 bytes) |

Build options (set in the [`Makefile`](../Makefile)): `LFS_NO_MALLOC`, `LFS_NO_ASSERT`, no debug, warning or error output, not thread safe, no migration from littlefs version 1. Because there is no heap, every open file needs a caller-supplied 128-byte cache; the helpers put the `lfs_file_t` and its cache on the stack.

Two consequences of `cache_size` = 128:

- A file of at most 128 bytes is stored inline in the metadata of its directory; a larger file occupies whole 4096-byte blocks. The keymap files (126 bytes) and the two counter files are always inline, the colour files `clr` (252 bytes) and `r<k>` (204 bytes on the 80HE, 168 on the 60HE+) never; for the protobuf files it depends on the stored content.
- [protocol](protocol.md) feature command 58 reports the number of blocks in use out of `BOARD_STORAGE_LFS_BLOCK_COUNT`.

### Block device

The four littlefs callbacks are in [`storage_bd.c`](../src/storage/storage_bd.c): `storage_bd_read`, `storage_bd_prog`, `storage_bd_erase`, `storage_bd_sync`. Block `b`, offset `o` maps to flash address `b * 4096 + o`.

- All four always return 0. A failing flash chip is not detected at this level; littlefs only notices through its own checksums and read-back comparisons.
- Every operation first waits until the chip is no longer busy (`nor_flash_wait_ready` in [`spi_nor.c`](../drivers/spi_nor/spi_nor.c)). That wait has no timeout and runs the USB device task while it waits.
- Each SPI command runs with interrupts masked (`hal_nor_bus_transfer_in`, `hal_nor_bus_transfer_out` of [`hal_nor_bus.h`](../hal/include/hal/hal_nor_bus.h)).

### Mounting

`storage_init` ([`storage_init.c`](../src/storage/storage_init.c)) is a start-up step of the storage module (`STARTUP_STEP(STARTUP_STORAGE, storage_init)`, order in [`startup.h`](../src/core/startup.h)) and runs right after the flash bus is set up:

1. If the bootloader left a request to format the file system (`hal_system_take_format_request` of [`hal_system.h`](../hal/include/hal/hal_system.h)), the request is cleared and the file system is formatted. The request is the value `0x420` in a backup register, which keeps its content across the reset that starts the firmware: `RTC_BKP1` on the 80HE, `BKP_DATA1` on the 60HE+. On the 80HE the factory bootloader writes it with its command 7 (settings reset), see [bootloader](bootloader.md); nothing in openwoot makes the request. Which program writes it on the 60HE+ is not known.
2. `lfs_mount`. If it fails: `lfs_format`, then `lfs_mount` again.
3. `g_storage_mounted` is set to 1 whatever the result of the second mount. KNOWN ISSUE: the flag therefore means "mounting was attempted", and after a failed second mount the file functions work on an unmounted file system. Matches stock firmware 2.14.1.

A file system that cannot be mounted is erased without asking. This also applies to a file system written by a newer littlefs with a higher on-disk version: this firmware and the stock firmware accept only disk version 2.0. If you move the firmware to a newer littlefs, pin the disk version to 2.0 or expect the stock firmware to format the volume when it is flashed back.

## Directories and file names

```
/
+-- 0/ 1/ 2/ 3/        built-in profile slots (namespace 0)
|     cfg rgb clr r1 r2 r3 ml0 ml1 ml2 ml3 i act prt prt2 ak2 gp gm lb lbm pst
+-- lp/
|   +-- 0/ ... 9/      second profile namespace (namespace 1)
|         the same files as above, plus al0 ... al15
+-- global  glb  glbm  sw  ds  boot_count
```

The files `lb`, `lbm`, `glb` and `glbm` belong to feature `lightbar`: only a board that lists it (the 80HE) reads or writes them.

- A profile is a directory. Its settings are split into one small file per topic, so changing one topic rewrites one file.
- Namespace 0 has the directories `0`..`3`, namespace 1 the directories `lp/<n>` for n = 0..9. The firmware counts profiles by probing these names in order and stopping at the first missing one (`storage_count_profile_dirs` in [`storage_profile_files.c`](../src/storage/storage_profile_files.c)), so directories must be consecutive. Namespace 1 is a set of profiles that only the host can create and select.
- A profile is identified by a 16-bit profile id: low byte index, high byte namespace (`profile_id_t` in [`profile_types.h`](../src/profile/profile_types.h)). `storage_format_profile_path` turns an id and a file name into `<index>/<name>` or `lp/<index>/<name>`. File names passed to it must be shorter than 16 characters and must not contain `%`, because the name goes through `sprintf` as a format string.
- Numbered files (`r<k>`, `ml<k>`, `al<k>`) are also probed in order; a gap ends the series.
- A missing file is never an error. Every loader falls back to defaults, and the "get" commands of the protocol answer with an empty payload.
- The active profile is not stored. `g_profile_active_id` is initialised to built-in profile 0, and the start-up step `rgb_start_and_activate_profile` ([`rgb_init.c`](../src/rgb/rgb_init.c)) activates that profile at every boot.

The layout was confirmed on a Wooting 80HE that had been used with the stock firmware: the file system has the [parameters](#parameters) given above, every file name on it is one of this document, the raw files have the sizes of the [file table](#file-table) (`clr` 252, `r<k>` 204, `ml<k>` 126, `pst` 4 bytes), every protobuf file is within its maximum size, and the last sector of the SPI flash holds the device-info record in the 1-byte format with the magic `0x1337`. That keyboard had the directories `0` to `3` and no `lp` directory.

## File table

`<dir>` stands for a profile directory (`<n>` or `lp/<n>`). "pb" means one nanopb-encoded protobuf message; "raw" means a fixed-size binary array. The maximum encoded size is the size of the encode buffer the firmware uses when it writes the file, which is the upper bound nanopb computes for the message (`<Message>_size` in [`settings.pb.h`](../src/proto/settings.pb.h)). Message links point at [`settings.proto`](../src/proto/settings.proto); search for the message name there.

The functions in the last two columns are the ones that decide to read or write; they go through the helpers described under [Write strategy](#write-strategy-and-its-consequences). `protocol_*` functions are the host command handlers listed in [protocol](protocol.md). `profile_activate` ([`profile_activate.c`](../src/profile/profile_activate.c)) reads the files of a profile through the profile sections of [`profile_sections.c`](../src/profile/profile_sections.c), which are registered with `PROFILE_SECTION` like the host commands; feature `lightbar` adds its own section for `lb`. The protobuf maximum sizes are the same on every board: the schema is shared, and its key matrices always have 6 x 21 cells.

### Files of a profile directory

| File | Format | Max. size (bytes) | Configures | Read by | Written by |
| --- | --- | --- | --- | --- | --- |
| `<dir>/cfg` | pb [`ProfileConfigFile`][m-cfg] | 26 | General settings: profile-wide actuation point, rapid trigger and its sensitivities, scan mode, tap duration, keyboard keys in gamepad mode | `profile_load_and_apply_cfg`, `protocol_cmd_get_profile_cfg` | `protocol_cmd_set_profile_cfg`, `protocol_cmd_save_live_cfg`, `profile_create_factory_defaults` |
| `<dir>/rgb` | pb [`RgbSettings`][m-rgb] | 182 (generated bound: 200, see [sizes](#encoded-sizes)) | Brightness, lock-indicator colours, effect layers. RAM copy `g_rgb_settings` | `profile_activate`, `protocol_cmd_get_rgb_settings` | `protocol_cmd_set_rgb_settings`, `protocol_cmd_save_live_rgb`, `rgb_save_settings_timer_cb` (2 s after a brightness key), `profile_create_factory_defaults` |
| `<dir>/clr` | raw, 252 bytes: `uint16[6][21]` RGB565 | 252 | Base colour of every key (the columns below `BOARD_KEY_COLS` are keys: 0..16 on the 80HE, 0..13 on the 60HE+). RAM copy `g_rgb_base_colors` | `profile_activate`, `protocol_cmd_get_base_colors_rows_0_2`, `protocol_cmd_get_base_colors_rows_3_5` | `storage_write_base_colors`, called by `protocol_cmd_set_base_colors_rows0_2`, `protocol_cmd_set_base_colors_rows3_5`, `protocol_cmd_save_live_rgb`, `profile_create_factory_defaults` |
| `<dir>/r<k>`, k = 1..3 | raw, `uint16[6][BOARD_KEY_COLS]` RGB565: `uint16[6][17]` on the 80HE, `uint16[6][14]` on the 60HE+ | 204 (80HE), 168 (60HE+) | Key colours shown while Fn layer k is active. RAM copy `g_rgb_layer_colors` | `storage_read_layer_colors`, called by `profile_activate`, `rgb_update_frame`, `protocol_cmd_get_layer_colors` | `storage_write_layer_colors`, called by `protocol_cmd_set_layer_colors` (which also deletes) and `profile_create_factory_defaults` (`r1`; on the 60HE+ also `r2`) |
| `<dir>/ml<k>`, k = 0..3 | raw, 126 bytes: `uint8[6][21]` key codes | 126 | Keymap of layer k. On the wire it travels as [`KeymapLayer`][m-keymap] | `storage_read_keymap_layer` and `layout_load_keymap_layers` ([`layout_keymap.c`](../src/layout/layout_keymap.c)), `protocol_cmd_get_keymap_layer` | `storage_write_keymap_layer`, called by `protocol_cmd_set_keymap_layer` (which also deletes) and `profile_create_factory_defaults` (profile 0, the board's default layers: 0 and 1 on the 80HE, 0..2 on the 60HE+) |
| `<dir>/i` | pb [`ProfileInfo`][m-info] | 65 | Profile name (30 characters), colour, and a 20-character string the firmware never interprets | `profile_activate` (colour only), `protocol_cmd_get_profile_info` | `protocol_cmd_set_profile_info`, `profile_create_factory_defaults` |
| `<dir>/act` | pb [`ActuationPoints`][m-act] | 519 | Per-key actuation points, as a dense matrix or a sparse list, 8-bit or 16-bit | `profile_activate`, `protocol_cmd_get_actuation_points` | `protocol_cmd_set_actuation_points` (also deletes), `profile_create_factory_defaults` |
| `<dir>/prt` | pb [`KeyByteMap`][m-bytemap] | 203 | Legacy per-key rapid-trigger mode. Superseded by `prt2`, but still read first | `profile_activate`, `protocol_cmd_get_legacy_rapid_trigger_keys` | `protocol_cmd_set_legacy_rapid_trigger_keys` (also deletes); deleted by `protocol_cmd_set_rapid_trigger_groups` |
| `<dir>/prt2` | pb [`RapidTriggerGroups`][m-rt] | 1320 | Rapid-trigger settings for up to 30 groups of keys. RAM copy `g_keys_rt_groups`. Read only if `prt` is absent | `profile_activate`, `protocol_cmd_get_rapid_trigger_groups` | `protocol_cmd_set_rapid_trigger_groups` (also deletes) |
| `<dir>/ak2` | pb [`AdvancedKeys`][m-ak] | 1040 | Up to 40 advanced-key bindings (dynamic keystroke, mod-tap, toggle, two-key bindings). RAM copy `g_keys_advanced_keys` | `profile_activate`, `protocol_cmd_get_advanced_keys` | `storage_save_advanced_keys`, called by `protocol_cmd_save_advanced_keys`, `protocol_cmd_set_advanced_keys`, `profile_create_factory_defaults` |
| `<dir>/dks` | not interpreted (legacy) | | Predecessor of `ak2`. This firmware never reads or writes it; `storage_save_advanced_keys` deletes it | none | none |
| `<dir>/gp` | pb [`GamepadSettings`][m-gp] | 45 | Analog stick response curve and stick options. RAM copy `g_hid_gamepad_settings` | `profile_activate`, `protocol_cmd_get_gamepad_settings` | `protocol_cmd_set_gamepad_settings` |
| `<dir>/gm` | pb [`GamepadMapping`][m-gm] | 208 | Gamepad control bound to each key | `profile_activate`, `protocol_cmd_get_gamepad_mapping` | `protocol_cmd_set_gamepad_mapping` (also deletes); deleted by `protocol_cmd_set_gamepad_settings` (known issue, see [protocol](protocol.md)) |
| `<dir>/lb` | pb [`LightbarConfig`][m-lb] | 838 | Feature `lightbar`. Up to 4 light-bar entries of the profile, enable flag, brightness. RAM copy `g_rgb_lightbar_config` | `lightbar_section_load` ([`lightbar.c`](../src/rgb/lightbar/lightbar.c)), run by `profile_activate`; `protocol_cmd_get_lightbar_config` | `protocol_cmd_set_lightbar_config`, `profile_create_factory_defaults` (through `lightbar_write_default_profile_file`) |
| `<dir>/lbm` | pb [`LbmFile`][m-lbm] | 136 | Feature `lightbar`. Up to four 30-character strings. Stored for the host only; no firmware code reads the content | `protocol_cmd_get_lbm_file` | `protocol_report_save_lbm` |
| `<dir>/pst` | raw, 4 bytes: `uint32` | 4 | [Revision counter](#revision-counter) of the directory | `protocol_cmd_get_profile_revision` | `storage_bump_profile_revision` |
| `lp/<n>/al<k>`, k = 0..15 | pb [`AlFile`][m-al] | 1332 | Strings of up to 260 characters that the firmware does not interpret; stored for the host only. Exists only in the `lp` namespace | `storage_count_al_files`, `protocol_cmd_get_al_file_count`, `protocol_cmd_get_profile_al_file` | `protocol_cmd_set_al_file` (also deletes) |

### Files in the root directory

| File | Format | Max. size (bytes) | Configures | Read by | Written by |
| --- | --- | --- | --- | --- | --- |
| `global` | pb [`GlobalSettings`][m-global] | 56 | Keyboard-wide settings: LED idle dimming and switch-off times, Num Lock indicator polarity, release-rebound filter. RAM copy `g_profile_global_settings` | `global_settings_load` ([`profile_init.c`](../src/profile/profile_init.c)) at boot, `protocol_cmd_get_global_settings` | `protocol_cmd_set_global_settings`, `profile_create_factory_defaults` |
| `glb` | pb [`GlobalLightbarConfig`][m-glb] | 828 | Feature `lightbar`. Up to 4 light-bar entries shared by all profiles. RAM copy `g_rgb_global_lightbar_config` | `rgb_load_global_lightbar_config` ([`lightbar.c`](../src/rgb/lightbar/lightbar.c)) at boot, `protocol_cmd_get_global_lightbar_config` | `protocol_cmd_set_global_lightbar_config`, `profile_create_factory_defaults` (through `lightbar_write_default_global_file`) |
| `glbm` | pb [`GlbmFile`][m-glbm] | 136 | Feature `lightbar`. Keyboard-wide counterpart of `lbm`; stored for the host only | `protocol_cmd_get_glbm` | `protocol_cmd_save_glbm` |
| `sw` | pb [`SwitchProfiles`][m-sw] | 1240 | Which hall switch type sits under which keys, with optional travel offsets. RAM copy `g_analog_switch_profiles` | `analog_load_switch_profiles` ([`analog_switch.c`](../src/analog/analog_switch.c)) at boot and for `protocol_cmd_get_switch_profiles` | `protocol_cmd_set_switch_profiles_body` |
| `ds` | pb [`DeviceSettings`][m-ds] | 8 | USB mode, NKRO or 6KRO keyboard report, forced full speed, gamepad polling rate. Read once at boot | `storage_load_device_settings`, called by `settings_load_and_apply`, `protocol_cmd_get_device_config`, `profile_create_factory_defaults` | `storage_save_device_settings`, called by `protocol_report_cmd_set_device_config` and the boot code |
| `boot_count` | raw, 4 bytes: `uint32` | 4 | Despite its name it is not incremented at boot: only the flash self-test command increments it | `protocol_cmd_flash_selftest` | `protocol_cmd_flash_selftest` |
| `lp` | directory | | Parent of the namespace-1 profile directories | `storage_count_profile_dirs` | created by `protocol_cmd_create_profile` |

### Encoded sizes

For every protobuf file except `rgb` the buffer size equals the `<Message>_size` constant that nanopb generates from the schema. For `rgb` the firmware uses 182 bytes (`PROTOCOL_RGB_FILE_BUFFER_SIZE`, `RGB_SETTINGS_FILE_BUF_SIZE`), the size stock firmware 2.14.1 uses, while the generated bound is 200. The difference comes from three one-byte fields of effect types that the firmware never reads: the schema declares them as integers, whose worst-case encoding is one byte longer than that of an enum, in each of the 6 effect layers. New code can use the generated constants everywhere; they are never too small.

## Write strategy and its consequences

### How a file is written

All writes go through two helpers.

`storage_write_file` ([`storage_file.c`](../src/storage/storage_file.c)) replaces a whole file:

1. `lfs_file_opencfg` with `LFS_O_WRONLY | LFS_O_CREAT | LFS_O_TRUNC` (`STORAGE_OPEN_REWRITE`) and a 128-byte cache on the stack.
2. One `lfs_file_write` with the complete content.
3. `lfs_file_close`. This is where littlefs commits the new content. The result of the close is ignored.

`storage_write_profile_pb_file` ([`storage_profile_files.c`](../src/storage/storage_profile_files.c)) is the wrapper for protobuf files of a profile:

1. Build the path.
2. Increment the revision counter of the profile (`storage_bump_profile_revision`).
3. Encode the message into a stack buffer of the maximum encoded size. The result of `pb_encode` is ignored.
4. `storage_write_file`.

Root files are encoded by their callers and written with `storage_write_file` directly. The raw files have their own small wrappers (`storage_write_base_colors`, `storage_write_layer_colors`, `storage_write_keymap_layer`), which also bump the revision first.

Reads: `storage_read_pb_file` opens the file, reads all of it into a stack buffer sized from the file size, closes it and decodes. `storage_read_file` reads a fixed number of bytes. Results: 1 (`STORAGE_OK`), -1 not mounted, -3 decode error, or the negative littlefs error truncated to 16 bits; -2 (`LFS_ERR_NOENT`) is the "file missing" case the callers test for. `storage_read_file` and `storage_write_file` return byte counts instead of 1.

### Revision counter

Every profile directory has a file `pst` with a 32-bit counter. `storage_bump_profile_revision` opens it with `LFS_O_RDWR | LFS_O_CREAT`, reads the value (0 if the file is new), adds 1, seeks to 0, writes and closes.

- It is incremented with every write or delete of a file in that directory that goes through the helpers: before the write, and before or after the delete depending on the handler. It is not incremented for root files.
- It is not incremented while the factory defaults are written (`g_storage_revision_bump_disabled`), so a fresh profile has no `pst` file and the "get revision" command answers with an empty payload.
- Some operations bump it more than once (saving the advanced keys bumps twice; replacing `prt` by `prt2` bumps twice).
- The host reads it with [protocol](protocol.md) feature command 72. It lets a host program detect that a stored profile changed without reading every file; the firmware itself never compares revisions.
- The counter is bumped before the data is written, so after an interrupted write the revision can have advanced although the data is unchanged.

### Consequences

- **Each save is a direct overwrite.** There is no temporary file, no rename and no read-back at file level. Atomicity of a single file comes from littlefs alone: by its design the old content stays valid until the close commits the new metadata, so after a power loss a file is either old or new. Power-loss behaviour is not yet tested on hardware.
- **There is no journal across files.** A change that touches several files is not atomic. Examples: the base colours travel as two commands (rows 0..2, rows 3..5) and each one rewrites `clr`; replacing the legacy rapid-trigger file removes `prt`, bumps the revision and then writes `prt2`. A power loss in between leaves a mixture that the loaders accept without complaint.
- **Errors are mostly dropped.** The encode result and the close result are ignored, most callers ignore the write result, and OUT-report commands have no response anyway. A full file system or a failed encode is not reported to the host. A failed encode stores the bytes produced so far; such a file may fail to decode later, in which case the loaders fall back to defaults.
- **Every save of a profile file costs at least two commits**: the `pst` update and the file itself. Each commit programs flash and may erase a block.
- **Saves block the main loop.** The flash commands run synchronously inside the command handler. Until the write is done the main loop does not run its other work (key processing, report sending, lighting); only the USB task is called from the flash wait loop, and interrupts are masked for the duration of each single SPI command. A sector erase takes tens of milliseconds on this class of chip (data-sheet figure).
- **Stack use is significant.** Writing a protobuf file puts the encode buffer on the stack (up to 1332 bytes for `al<k>`, 1320 for `prt2`) in addition to the decoded message and the 128-byte file cache; reading puts the whole file on the stack. A corrupted directory entry with a large file size would make `storage_read_pb_file` allocate that much stack.
- **Handles leak on some error paths.** KNOWN ISSUE: `storage_bump_profile_revision`, `storage_delete_profile_dir`, `protocol_cmd_get_profile_revision` and `protocol_cmd_flash_selftest` return without closing when a step after the open fails; the open-file list of `g_storage_lfs` then points into a dead stack frame. Matches stock firmware 2.14.1.
- **Unknown fields are skipped, unknown files are ignored.** nanopb skips tags it does not know, and nothing enumerates the files of a profile except the delete function, which removes every file of the directory. A modified firmware can therefore add its own files to a profile directory, or new optional fields to a message, without confusing the stock firmware; they are removed together with the profile. Not yet tested on hardware.

## First boot, defaults and factory reset

### First boot

`storage_create_first_boot_profiles` ([`profile_first_boot.c`](../src/profile/profile_first_boot.c)) is the start-up step that runs right after mounting. If not a single built-in profile directory exists (directory `0` is missing), it treats the boot as the first one:

1. Create one directory per factory profile (`profile_edition_factory_profile_count`): `0` and `1`, on a unit of the 80HE's TenZ edition also `2`.
2. Call `profile_create_factory_defaults` with the revision counter disabled.

`profile_create_factory_defaults` is board code: each board's [`defaults/profile_defaults.c`](../boards/wooting_80he/defaults/profile_defaults.c) ([60HE+](../boards/wooting_60he_plus/defaults/profile_defaults.c)) describes its factory profiles as data and writes the files in its own order with the helpers of [`profile_default_files.c`](../src/profile/profile_default_files.c); the colour maps are in the board's `tables/profile_tables.c`. It writes each default file only if it does not exist yet, so existing files are kept. Per created profile it writes:

| File | 80HE, regular unit | 80HE, TenZ edition | 60HE+ |
| --- | --- | --- | --- |
| `rgb` | profile 0: rainbow wave under two reactive fades; profile 1: rainbow wave and key test | profiles 0 and 2: no effect, brightness 112; profile 1: rainbow wave under two reactive fades | profile 0: full-circle rainbow wave under a white reactive fade; profile 1: rainbow wave and key test |
| `clr` | profile 0: `k_profile_default_colors_a`; profile 1: `k_profile_default_colors_b` | profile 1: table a; profiles 0 and 2: table b | profile 0: table a; profile 1: table b (the board's own tables) |
| `r1` | `k_profile_default_layer1_colors` | the same | `k_profile_default_layer1_colors` of the board |
| `r2` | not written | not written | `k_profile_default_layer2_colors` |
| `cfg` | profile 0: actuation point 63/255 of the travel, no rapid trigger; profile 1: actuation point 26/255, rapid trigger on with sensitivity 26/255 | profile 1 as regular profile 0; profiles 0 and 2: rapid trigger on, actuation point 63/255; profile 2 additionally the parallel scan mode | profile 0: actuation point 102/255, no rapid trigger; profile 1: actuation point 26/255, rapid trigger on with press sensitivity 13/255 and release sensitivity 35/255 |
| `act` | not written | profiles 0 and 2: per-key actuation points for nine keys | not written |
| `i` | `Typing Profile` (profile 0), `Rapid Profile` (profile 1) | `TenZ Profile` (0), `Typing Profile` (1), `TenZ Ultra Profile` (2) | `Typing Profile` (0), `Rapid Profile` (1) |
| `ak2` | not written | profile 2: two two-key bindings | not written |
| `lb` | not written | profiles 0 and 2 | not written (no feature `lightbar`) |
| `ml<k>` | profile 0 only: the factory keymaps of layers 0 and 1 (`k_layout_default_keymap_layers`), with two key codes replaced on a JIS unit | the same | profile 0 only: the factory keymaps of layers 0, 1 and 2 (`BOARD_DEFAULT_KEYMAP_LAYERS` = 3), no replacement |

After the profiles it writes the root file `global` (idle dimming after 5 minutes, LEDs off after 10 minutes) and, on the 80HE, `glb` (one light-bar entry: a profile indicator over all 10 LEDs).

Not created at first boot: `gp`, `gm`, `prt`, `prt2`, `lbm`, `pst`, `sw`, `ds`, `glbm`, `boot_count`. They appear when the host writes them.

### Defaults when a file is missing

The loaders do not depend on the files written at first boot. `profile_activate` ([`profile_activate.c`](../src/profile/profile_activate.c)) and `profile_load_and_apply_cfg` ([`profile_cfg.c`](../src/profile/profile_cfg.c)) apply these fallbacks:

| File | Fallback |
| --- | --- |
| `cfg` | the `cfg` of built-in profile 0; if that is missing too, `profile_fill_default_cfg` for the profile index |
| `rgb` | `profile_fill_default_rgb_settings` for the profile index (`lp` profiles use index 0) |
| `clr` | `profile_copy_default_base_colors` for the profile index |
| `lb` | (feature `lightbar`) light bar enabled, global entries only, brightness 255 |
| `r<k>` | no layer colours |
| `act`, `prt`, `prt2`, `ak2`, `gm` | no per-key overrides, no advanced keys, no gamepad bindings |
| `ml0` | the `ml0` of built-in profile 0, then the factory keymap. Further layers come from the same profile as layer 0 |
| `global` | idle dimming after 5 minutes, off after 10 minutes, but with the "present" flags clear, so the idle timer stays off until the host writes the settings |
| `sw` | no entries, default switch type (`profile_edition_default_switch_type`: 0, or 1 on a TenZ unit of the 80HE) |
| `ds` | USB mode 2 (no gamepad interface), NKRO, high speed allowed, gamepad polling rate 1 ms |

So a profile directory created by the host (feature command 64) is usable while still empty.

### Factory reset

Three paths erase the file system. All of them call `lfs_format`, which leaves the device-info record and the serial mirror alone, and all of them end in a first boot as described above.

| Trigger | Code | Notes |
| --- | --- | --- |
| [protocol](protocol.md) feature command 43 | `protocol_cmd_factory_reset` ([`protocol_device.c`](../src/protocol/protocol_device.c)) | formats, waits 100 ms, resets the MCU; no response is sent |
| format request of the bootloader at boot (`0x420` in `RTC_BKP1` on the 80HE, `BKP_DATA1` on the 60HE+) | `storage_init` | on the 80HE requested by command 7 (settings reset) of the factory bootloader, see [Mounting](#mounting) |
| the file system does not mount | `storage_init` | automatic |

After a factory reset the device settings are back at their defaults as well (the file `ds` is gone), so a keyboard that was in XInput or gamepad mode enumerates with product id `BOARD_USB_PID_BASE` + 2 (`0x1402` on the 80HE, `0x1322` on the 60HE+) at the next boot. The serial number, the edition and the lock mode survive, because they are in the device-info record; so does the physical layout (device-info record on the 80HE, option byte on the 60HE+).

Deleting a single profile (feature command 63, `storage_delete_profile_dir`) removes every file of its directory and then the directory. Built-in profile 0 cannot be deleted.

## Differences between the boards

The file system, the file formats, the protobuf schema and the record formats are the same on every board. What differs follows from the board's `board.h` ([80HE](../boards/wooting_80he/board.h), [60HE+](../boards/wooting_60he_plus/board.h)) and from the features and strategy variants its `board.mk` selects ([80HE](../boards/wooting_80he/board.mk), [60HE+](../boards/wooting_60he_plus/board.mk)). The values of the 60HE+ are taken from that board's stock firmware, not confirmed on hardware.

| | Wooting 80HE | Wooting 60HE+ |
| --- | --- | --- |
| littlefs `block_count` (`BOARD_STORAGE_LFS_BLOCK_COUNT`) | 2047: SPI flash `0x000000`-`0x7FEFFF` | 2048: the whole SPI flash |
| Bus of the SPI flash (family) | [`hal/gd32f4xx/hal_nor_bus.c`](../hal/gd32f4xx/hal_nor_bus.c) | [`hal/gd32f30x/hal_nor_bus.c`](../hal/gd32f30x/hal_nor_bus.c) |
| Device-info record (`STRATEGY_device_info`, `BOARD_DEVICE_INFO_ADDR`) | `nor_flash_sector`: last sector of the SPI flash, `0x7FF000`; read and written | `mcu_flash_page_readonly`: MCU flash page `0x080FF000`; read only, storing changes nothing; an older raw record is accepted |
| Physical layout (`STRATEGY_layout_variant`, `k_layout_variants`) | `device_info`: field of the device-info record; ANSI, ISO, JIS | `mcu_option`: bit 0 of the user option byte; ANSI, ISO; never written |
| Serial number page (`BOARD_MCU_SERIAL_PAGE_ADDR`) | `0x080FE000`; written at boot when blank and after OUT-report command 12 | `0x080FD000`; never written |
| Format request of the bootloader | `0x420` in `RTC_BKP1` | `0x420` in `BKP_DATA1` |
| Key maps `ml<k>` and base colours `clr` (`BOARD_KEY_ROWS` x `BOARD_KEYMAP_COLS`) | 126 and 252 bytes (6 x 21) | 126 and 252 bytes (6 x 21) |
| Layer colours `r<k>` (`BOARD_KEY_ROWS` x `BOARD_KEY_COLS` x 2 bytes) | 204 bytes (6 x 17) | 168 bytes (6 x 14) |
| Protobuf files | maximum sizes of the [file table](#file-table) | the same |
| Light-bar files `lb`, `glb`, `lbm`, `glbm` (feature `lightbar`) | read and written | never read or written; their host commands are not registered |
| Edition (feature `edition`) | TenZ edition for serial `edition` 9 or 99 | none: every unit is regular |
| Factory profiles created at first boot | 2; 3 on a TenZ unit | 2 |
| Default files written at first boot (beyond `rgb`, `clr`, `cfg`, `i`) | `r1`; `ml0`, `ml1`; `act`, `ak2`, `lb` on a TenZ unit; root files `global`, `glb` | `r1`, `r2`; `ml0`, `ml1`, `ml2`; root file `global` |
| Product id after a factory reset | `0x1402` | `0x1322` |

## The protobuf schema

All structured data, in the files and in the payloads of the host protocol, is Protocol Buffers in the proto2 wire format, encoded and decoded with nanopb.

| File | Content |
| --- | --- |
| [`src/proto/settings.proto`](../src/proto/settings.proto) | the schema: one proto2 file with 106 messages and 18 enums |
| [`src/proto/settings.options`](../src/proto/settings.options) | nanopb options: static allocation for every field, string and array limits, integer widths. They fix the layout of the C structs. |
| [`src/proto/settings.pb.h`](../src/proto/settings.pb.h), [`settings.pb.c`](../src/proto/settings.pb.c) | C structs, field descriptors and `<Message>_size` constants, generated from the two files above with the nanopb generator 0.4.9.1 |

Field numbers, wire types, `required` / `optional` / `repeated`, `oneof` membership, limits and default values are fixed by compatibility with stock firmware 2.14.1 and with the vendor's configuration program. Names are not part of the wire format: renaming a message or field changes the generated C identifiers and nothing else.

### Messages stored in files

| Message | File | Content |
| --- | --- | --- |
| `ProfileConfigFile` | `cfg` | wrapper around `ProfileConfig`: profile-wide actuation point, rapid trigger with press and release sensitivity, scan mode (`ScanMode`), tap duration (`TapDuration`), keyboard keys in gamepad mode |
| `RgbSettings` | `rgb` | `RgbBaseSettings` (brightness, lock-indicator colours), then either one `RgbLegacyEffect` or an `RgbEffectLayerList`. Each `RgbEffectLayer` selects one effect kind (`RgbEffectKind`) with its parameter message `RgbEffect...` (rainbow wave, breathing, hue cycle, scanner, reactive fade, key ripple, level meter, analog effects, polar gradient, simplex noise, key test). A legacy effect is converted into a one-entry layer list when it is loaded. |
| `ProfileInfo` | `i` | profile name, colour (`RgbColor`), one string the firmware does not interpret |
| `ActuationPoints` | `act` | per-key actuation points in one of four encodings: 6 x 21 matrix or list of up to 50 keys, with 8-bit or 16-bit values (`KeyMatrixU8`, `KeyValueListU8`, `KeyMatrixU16`, `KeyValueListU16`) |
| `KeyByteMap` | `prt`, and inside `GamepadMapping` | one byte per key as a matrix or a list |
| `RapidTriggerGroups` | `prt2` | up to 30 `RapidTriggerGroup` entries: a set of keys with `RapidTriggerParams` |
| `AdvancedKeys` | `ak2` | up to 40 `AdvancedKey` entries, each one of `DynamicKeystroke`, `ModTap`, `ToggleKey`, `TwoKeySocd`, `TwoKeyDepthPriority` |
| `GamepadSettings` | `gp` | `GamepadResponseCurve` (curve points) and `GamepadStickOptions` (among them the stick normalisation, `StickNormalization`) |
| `GamepadMapping` | `gm` | gamepad control per key (`KeyByteMap`) and whether a bound key still sends its key code |
| `LightbarConfig` | `lb` | enable flag, up to 4 `LightbarEntry`, "also use the global entries" flag, brightness |
| `GlobalLightbarConfig` | `glb` | up to 4 `LightbarEntry` shared by all profiles. An entry covers a range of the 10 light-bar LEDs (`LightbarRange`), has a condition built from signal comparisons (`LightbarConditionGroup`, `LightbarConditionTerm`, `LightbarCompare`, `SignalRef`), and one display kind: `LightbarStatic`, `LightbarBlink`, `LightbarCustom`, `LightbarMirror` or a profile indicator. |
| `LbmFile`, `GlbmFile` | `lbm`, `glbm` | up to four 30-character strings, stored for the host only |
| `AlFile` | `al<k>` | strings of up to 260 characters, stored for the host only |
| `GlobalSettings` | `global` | `IdleSettings` (minutes until the LEDs dim and switch off), Num Lock indicator polarity, release-rebound filter, one string the firmware does not interpret |
| `SwitchProfiles` | `sw` | up to 30 `SwitchProfileEntry` (a `SwitchKeyGroup` of keys with `SwitchParams`: switch type `SwitchType` and travel offsets), default parameters for all other keys, a format version |
| `DeviceSettings` | `ds` | NKRO or 6KRO, USB mode (`UsbGamepadMode`), forced full speed, gamepad polling rate (`GamepadPollInterval`) |
| `DeviceInfo` | device-info record | `SerialNumber`, `DeviceInfoAux`, `PhysicalLayout`, lock mode; see [The device-info record](#the-device-info-record) |

### Messages used only by the host protocol

| Message | Used by | Content |
| --- | --- | --- |
| `KeymapLayer` | feature command 48, OUT-report command 20 | the 6 x 21 key codes of one layer; the file `ml<k>` stores the same bytes without protobuf framing |
| `ColorRows` | commands for the base colours and the layer colours | three rows (`ColorRow`) of 21 RGB565 colours |
| `KeyMatrixU8` | feature command 20 | per-key sensor values |
| `AdvancedKeysPage` | feature command 52, OUT-report command 24 | up to 10 `AdvancedKey` entries, the index of the first one and the total count |
| `StorageUsage` | feature command 58 | file-system blocks in use |
| `ActiveProfileState` | feature command 73 | active profile, SDK mode, LEDs off by idle timer |
| `HostSignalList`, `HostSignalValues` | feature command 65, OUT-report command 31 | host-defined signals of the light bar as {index, id} and {index, value} |
| `LightbarColors` | feature command 68, OUT-report command 30 | colours of the 10 light-bar LEDs |
| `SwitchTypeList` | feature command 83 | `SwitchTypeEntry` per known switch type |
| `Event` | unsolicited input reports | a `oneof` whose tag is the event number: `ProfileActivatedEvent`, `LightbarChangedEvent`, `ProfilesChangedEvent` |

The command numbers and frames are in [protocol](protocol.md).

### Names and placeholders

Most names describe what the firmware does with the message or field (`ProfileConfig.actuation_point`, `LightbarCompare.comparator`). Content that the firmware only stores, forwards or never uses has placeholder names: `f<tag>` for a field and `o<tag>` for a `oneof`; a message or enum that consists only of such content has a neutral name. The placeholders:

| Placeholder | What the firmware does with it |
| --- | --- |
| members of `AlFile` and its sub-messages | files `al<k>`: moved between host and flash only |
| `DeviceInfo.f2`, members of `DeviceInfoAux` and its sub-messages | stored and returned only |
| `SerialNumber.f1`..`f6`, `f10`, `f11` | numbers printed into the serial string, see [The device-info record](#the-device-info-record) |
| `RgbEffectLayer.f12`, `.f13` and their messages | effect kinds 12 and 13: never drawn |
| `RgbEffectLevelMeter.f3`..`f5`, `RgbLegacyEffect.f6` | not used |
| `GlobalSettings.f4`, `ProfileInfo.f3` | strings without a reader in the firmware. The factory profiles store `woot:p:black`, `switchy:laser`, `switchy:gamer` and `switchy:gamerUltra` in `ProfileInfo.f3` |
| `LbmFile.f1`, `GlbmFile.f1` and their element message | up to four 30-character strings, stored and returned only |
| `SwitchTypeEntry.f2`, `.f3`, `.f5` | constants sent to the host and used nowhere else |
| `Event.f3` and its messages | event 3 is never built |

A placeholder can be given a better name at any time without touching stored data or host compatibility.

### Regenerating the C code

The files `settings.pb.h` and `settings.pb.c` are generated and checked in; the build does not run the generator. After a change to the `.proto` or `.options` file, regenerate them from the repository root. This needs the nanopb generator 0.4.9.1 (`python3 -m pip install nanopb==0.4.9.1`) and the protobuf compiler (protoc) on the `PATH`:

```sh
nanopb_generator -I src/proto -f src/proto/settings.options -D src/proto src/proto/settings.proto
```

With an unchanged schema this reproduces the checked-in files. [`src/proto/README.md`](../src/proto/README.md) has the same instructions next to the schema.

The generated header needs `pb.h` of nanopb 0.4.x, which is in `third_party/nanopb/`. The firmware is built with `-DPB_NO_ERRMSG -DPB_WITHOUT_64BIT` (see the [`Makefile`](../Makefile)); the generated code relies on the same options.

### Changing the schema

These points follow from how nanopb and the storage code work; they are not yet tested on hardware.

- Adding an `optional` field with a new tag to a stored message keeps old files readable (the field is absent and takes its default) and keeps new files readable by the stock firmware (it skips the unknown tag). Adding a `required` field breaks reading of old files.
- A changed message changes the size of its C struct. Many of these structs are RAM globals or sit inside other structs with `_Static_assert` checks on their size (the `*_types.h` headers under `src/`); those assertions show where the layout is relied upon.
- Size the encode buffer of a file with the generated `<Message>_size` constant, not with a copied number.
- The vendor's configuration program decodes the same messages. A message it reads through the "get" commands should keep the meaning of its existing tags.
- A new setting that the vendor's configuration program should never see is simpler to store in a new file of your own than in an existing message.

[m-cfg]: ../src/proto/settings.proto
[m-rgb]: ../src/proto/settings.proto
[m-global]: ../src/proto/settings.proto
[m-info]: ../src/proto/settings.proto
[m-lb]: ../src/proto/settings.proto
[m-act]: ../src/proto/settings.proto
[m-bytemap]: ../src/proto/settings.proto
[m-rt]: ../src/proto/settings.proto
[m-ak]: ../src/proto/settings.proto
[m-gp]: ../src/proto/settings.proto
[m-gm]: ../src/proto/settings.proto
[m-al]: ../src/proto/settings.proto
[m-devinfo]: ../src/proto/settings.proto
[m-glb]: ../src/proto/settings.proto
[m-aux]: ../src/proto/settings.proto
[m-serial]: ../src/proto/settings.proto
[m-keymap]: ../src/proto/settings.proto
[m-lbm]: ../src/proto/settings.proto
[m-glbm]: ../src/proto/settings.proto
[m-sw]: ../src/proto/settings.proto
[m-ds]: ../src/proto/settings.proto
