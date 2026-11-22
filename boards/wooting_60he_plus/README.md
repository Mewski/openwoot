# Wooting 60HE+

Everything in openwoot that is specific to the Wooting 60HE+: the description of the board, its
tables, its factory defaults and its reference data. The firmware is in [`src/`](../../src/), the
code for the microcontroller family in [`hal/gd32f30x/`](../../hal/gd32f30x/), the chip drivers in
[`drivers/`](../../drivers/). [docs/architecture.md](../../docs/architecture.md) explains how the
parts fit together, [docs/building.md](../../docs/building.md) how the build combines them and
[docs/porting.md](../../docs/porting.md) what a board directory must provide.

```sh
make BOARD=wooting_60he_plus
```

## State

**This firmware has never run on a keyboard. Do not flash it.**

- The firmware builds and links (`LTO=0` and `LTO=1`), and `make check` covers it.
- Its code was run function by function on a simulated microcontroller next to stock firmware
  2.14.1 of this board, with the same inputs, and the results were compared. That covers the
  logic: key processing, lighting, settings, host commands. It does not show that the hardware
  facts are right: pins, clocks, the scan timing, the LED wiring and the response of the chips are
  not confirmed on a device.
- The behaviour of this board's factory bootloader is not known: what it checks before it starts
  an application, how its update mode works, how a keyboard is brought back after a bad image.
  There is no flashing procedure and no recovery procedure. `board.json` says so
  (`"supported": false`), and `tools/openwoot.py` refuses `enter` and `flash` for this board;
  `make BOARD=wooting_60he_plus flash`, which runs these two commands, fails as well.
- openwoot never erases or programs the flash of this board's microcontroller: the family's
  `hal_mcu_flash_write_page` changes nothing, and the flash controller driver of the peripheral
  library is not linked. The factory device information and the serial number page are read-only;
  the host commands that would store them change nothing permanently.

## What the firmware consists of

[`board.mk`](board.mk):

| Item | Value |
| --- | --- |
| Family | `gd32f30x`, device `GD32F30X_XD` |
| Drivers | `is31fl3741` (LED matrix), `spi_nor` (settings flash) |
| Features | `reference` |
| Strategies | `noise_recal`: `stddev`; `device_info`: `mcu_flash_page_readonly`; `layout_variant`: `mcu_option` |

## Files

Every board directory has the same files (`tools/check_tree.py` compares them).

| File | Content |
| --- | --- |
| [`board.mk`](board.mk) | What the build combines for this board (table above) |
| [`board.h`](board.h) | The board in numbers, as `BOARD_` constants: identity, image, key matrix, scan, converter, layout, lighting, LED driver, storage. The same constants in the same order on every board; this board lists no feature that needs constants. |
| [`wiring.h`](wiring.h) | The pins, in the vocabulary of the family. Read by the code in `hal/gd32f30x/` only; the list of constants is [`hal/gd32f30x/wiring_template.h`](../../hal/gd32f30x/wiring_template.h). |
| [`board.json`](board.json) | What the host tool `tools/openwoot.py` needs: USB ids, image limits, and that the update procedure of the board is not known. It repeats values of `board.h`; `make check` compares them. |
| [`linker.ld`](linker.ld) | Memory regions of the application image; the section layout is `hal/gd32f30x/sections.ld` |
| [`tables/adc_scan_tables.c`](tables/adc_scan_tables.c) | The order of the multiplexers when all are converted together |
| [`tables/layout_tables.c`](tables/layout_tables.c) | Factory key maps of layers 0, 1 and 2; per layout variant (ANSI, ISO) the maps between scan positions and key ids and the space-bar fill columns; the variant table `k_layout_variants` |
| [`tables/rgb_tables.c`](tables/rgb_tables.c) | Key-to-LED maps for ANSI and ISO; `k_rgb_led_channels` (where each LED number sits in the LED frame) and `k_rgb_frame_pages` (which driver registers each frame page covers) |
| [`tables/profile_tables.c`](tables/profile_tables.c) | Factory colour maps of the default profiles and of layers 1 and 2 |
| [`defaults/profile_defaults.c`](defaults/profile_defaults.c) | The factory profiles and the order in which the first boot writes the default files |
| [`layout/key_positions_ansi.csv`](layout/key_positions_ansi.csv), [`key_positions_iso.csv`](layout/key_positions_iso.csv) | Position, size, LED number and default key code of every key, per physical layout. Reference data, not compiled. |
| [`layout/leds.csv`](layout/leds.csv) | Position of every LED, per physical layout. Reference data, not compiled. |

Every table has a comment at its definition; the declarations are in the header of the same name.

The last column of every CSV line, `position_source`, says where the position comes from:

| Value | Meaning |
| --- | --- |
| `nominal` | (key files) No mechanical data of this board is available. Row, column, LED number and key code are those of the tables; the position and the size are those of the usual 60 % key grid (1 unit = 19.05 mm, origin at the centre of the first key of row 1). They are not measurements. |
| `assumed_under_key` | (`leds.csv`) The LED is listed with the nominal position of its key |
| `unknown` | (`leds.csv`) The LED belongs to a matrix position without a key (under the space bar); no position is given |

## The board

None of these facts is confirmed on a device.

| Property | Value |
| --- | --- |
| Microcontroller | GigaDevice GD32F303 (ARM Cortex-M4F) with 1024 KiB flash and 96 KiB RAM; internal 8 MHz oscillator, no crystal in use, system clock 120 MHz |
| Flash layout | Factory bootloader at `0x08000000`-`0x08006FFF`; application from `0x08007000`; serial number page at `0x080FD000`, factory device information at `0x080FF000` |
| Key matrix | 6 rows x 14 columns (84 positions), row 0 without keys; key id = row << 5 \| column. Key maps and base colour maps are stored with 21 columns per row. |
| Physical layouts | ANSI (61 keys) and ISO (62 keys): two circuit boards, one firmware. Bit 0 of the user option byte selects the tables at start-up (strategy `mcu_option`). |
| Sensors | One hall-effect sensor per key, read through four 16:1 analog multiplexers: 4 shared address lines (PB0, PB1, PB10, PB11), one enable line (PB8, PB9, PC13, PC14) and one converter input (PA0..PA3) per multiplexer. 64 scan positions, addresses 0..15 in use. A second converter measures a zero-field reference input (PA4) together with every sensor conversion (feature `reference`); the converter averages 8 conversions per sample. |
| LEDs | One IS31FL3741 matrix driver on `I2C0` (PB6, PB7; enable output PB5), switch lines 1..6 in use. LED numbers 0..107, of which 66 carry an RGB LED: 65 (ANSI) or 66 (ISO) under keys. No light bar. |
| Settings storage | 8 MiB SPI NOR flash on `SPI1` (MOSI PB15, SCK PB13, MISO PB14, chip select PB12), the whole chip used by the file system |
| USB | Full-speed device controller of the microcontroller; the D+ pull-up is switched by PA15 |
| USB ids | Vendor id `0x31E3`. Product id `0x1320` + USB mode: `0x1320` with the XInput interface, `0x1321` with the HID gamepad interface, `0x1322` without gamepad interface (the default). |
| USB strings | Manufacturer "Wooting", product "Wooting 60HE+" |

Pins that are not in use stay in their reset state; PA13 and PA14 remain the debug port.
