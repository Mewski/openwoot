# Wooting 80HE

Everything in openwoot that is specific to the Wooting 80HE: the description of the board, its
tables, its factory defaults and its reference data. The firmware is in [`src/`](../../src/), the
code for the microcontroller family in [`hal/gd32f4xx/`](../../hal/gd32f4xx/), the chip drivers in
[`drivers/`](../../drivers/). [docs/architecture.md](../../docs/architecture.md) explains how the
parts fit together, [docs/building.md](../../docs/building.md) how the build combines them and
[docs/porting.md](../../docs/porting.md) what a board directory must provide.

```sh
make BOARD=wooting_80he
```

## State

- An earlier form of this firmware ran on one Wooting 80HE (ANSI layout): USB enumeration, host
  queries, lighting, typing, Fn combinations, profile switching, rapid trigger.
- The tree has been restructured since (layers, HAL interface, drivers, features, strategies). The
  restructured build has not yet been run on hardware.
- Not yet tested on hardware: the `LTO=1` build, gamepad and XInput modes,
  the vendor's configuration program against this firmware, suspend and resume, long-term
  stability, ISO and JIS boards.
- The update procedure and the recovery key combination are confirmed on a Wooting 80HE;
  [docs/flashing.md](../../docs/flashing.md) describes them.

## What the firmware consists of

[`board.mk`](board.mk):

| Item | Value |
| --- | --- |
| Family | `gd32f4xx`, device `GD32F425` |
| Drivers | `is31fl3741` (LED matrix), `spi_nor` (settings flash) |
| Features | `signals`, `lightbar`, `edition`, `recheck` |
| Strategies | `noise_recal`: `minmax`; `device_info`: `nor_flash_sector`; `layout_variant`: `device_info` |

## Files

Every board directory has the same files (`tools/check_tree.py` compares them).

| File | Content |
| --- | --- |
| [`board.mk`](board.mk) | What the build combines for this board (table above) |
| [`board.h`](board.h) | The board in numbers, as `BOARD_` constants: identity, image, key matrix, scan, converter, layout, lighting, LED driver, storage; then the blocks of features `lightbar`, `edition` and `recheck`. The same constants in the same order on every board. |
| [`wiring.h`](wiring.h) | The pins, in the vocabulary of the family. Read by the code in `hal/gd32f4xx/` only; the list of constants is [`hal/gd32f4xx/wiring_template.h`](../../hal/gd32f4xx/wiring_template.h). |
| [`board.json`](board.json) | What the host tool `tools/openwoot.py` needs: USB ids, image limits, the update mode's product id and block size. It repeats values of `board.h`; `make check` compares them. |
| [`linker.ld`](linker.ld) | Memory regions of the application image; the section layout is `hal/gd32f4xx/sections.ld` |
| [`tables/adc_scan_tables.c`](tables/adc_scan_tables.c) | The order of the multiplexers when all are converted together |
| [`tables/layout_tables.c`](tables/layout_tables.c) | Factory key maps of layers 0 and 1; per layout variant (ANSI, ISO, JIS) the maps between scan positions and key ids, the space-bar fill columns and the key codes the variant writes into layer 0; the variant table `k_layout_variants` |
| [`tables/rgb_tables.c`](tables/rgb_tables.c) | Key-to-LED maps for ANSI, ISO and JIS; `k_rgb_led_channels` (where each LED number sits in the LED frame) and `k_rgb_frame_pages` (which driver registers each frame page covers) |
| [`tables/profile_tables.c`](tables/profile_tables.c) | Factory colour maps of the default profiles and of layer 1 |
| [`defaults/profile_defaults.c`](defaults/profile_defaults.c) | The factory profiles of a regular unit and of a TenZ edition unit (feature `edition`), the factory content of the light-bar files (feature `lightbar`), and the order in which the first boot writes the default files |
| [`layout/key_positions_ansi.csv`](layout/key_positions_ansi.csv), [`key_positions_iso.csv`](layout/key_positions_iso.csv), [`key_positions_jis.csv`](layout/key_positions_jis.csv) | Position, size, LED number and default key code of every key, per physical layout. Reference data, not compiled. |
| [`layout/leds.csv`](layout/leds.csv) | Position of every LED, per physical layout, and of the light-bar LEDs. Reference data, not compiled. |

Every table has a comment at its definition; the declarations are in the header of the same name.

The last column of every CSV line, `position_source`, says where the position comes from:

| Value | Meaning |
| --- | --- |
| `design_files` | Measured in the mechanical design files of the keyboard |
| `under_key_design_files` | (`leds.csv`) The position given is the switch centre of the key above the LED, from the design files; the LED window is 4.94 mm away from it, see [docs/layout.md](../../docs/layout.md) |
| `design_files_order_assumed` | (`leds.csv`, light bar) The ten positions are from the design files; that light-bar position 0 is the left end is an assumption |

## The board

| Property | Value |
| --- | --- |
| Microcontroller | GigaDevice GD32F425 (ARM Cortex-M4F), 25 MHz crystal, system clock 200 MHz, at least 1024 KiB flash, 192 KiB RAM |
| Flash layout | Factory bootloader at `0x08000000`-`0x08007FFF`; application from `0x08008000`; serial number page at `0x080FE000` |
| Key matrix | 6 rows x 17 columns (102 positions); key id = row << 5 \| column. Key maps and base colour maps are stored with 21 columns per row. |
| Physical layouts | ANSI (84 keys), ISO (85 keys), JIS (88 keys): three circuit boards, one firmware. The layout stored in the factory device information selects the tables at start-up. |
| Sensors | One hall-effect sensor per key, read through six 16:1 analog multiplexers: 4 shared address lines (PA8..PA11), one enable line and one converter input per multiplexer. 96 scan positions, addresses 0..14 in use. Zero-field level assumed at 2048; inputs 3 to 5 are converted a second time when their reading changes (feature `recheck`). |
| LEDs | One IS31FL3741 matrix driver on `I2C0` (PB6, PB7; enable output PB8). LED numbers 1..104 on 8 switch lines of 13: 88 (ANSI), 89 (ISO) or 90 (JIS) key LEDs and 10 light-bar LEDs. |
| Settings storage | 8 MiB SPI NOR flash on `SPI0` (MOSI PA7, SCK PB3, MISO PB4, chip select PD2); the factory device information is in its last sector |
| USB | High-speed device through an external ULPI transceiver |
| USB ids | Vendor id `0x31E3`. Product id `0x1400` + USB mode: `0x1400` with the XInput interface, `0x1401` with the HID gamepad interface, `0x1402` without gamepad interface (the default). The factory bootloader in update mode has product id `0x140F`. |
| USB strings | Manufacturer "Wooting", product "Wooting 80HE" ("Wooting 80HE TenZ" on the TenZ edition) |

[docs/hardware.md](../../docs/hardware.md) describes the electronics as the firmware uses them:
clocks, peripherals, the complete pin map, the scan timing, the LED driver.
[docs/layout.md](../../docs/layout.md) gives the position of every key and LED.
