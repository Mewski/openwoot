# Key and LED layout

This document gives the position of every key switch and every LED of each supported keyboard, for each physical variant, and shows how these positions relate to the key ids, scan positions and LED numbers the firmware works with.

| Board | Variants | Source of the positions |
| --- | --- | --- |
| [Wooting 80HE](#wooting-80he) | ANSI, ISO, JIS | Wooting's published mechanical design files for the 80HE (<https://github.com/WootingKb/wooting-design>), licensed CERN-OHL-S. Measured positions. |
| [Wooting 60HE+](#wooting-60he) | ANSI, ISO | No mechanical data of this board is available. The positions are **nominal**: the usual 60 % key grid (1u = 19.05 mm) laid over the key rows and columns of the firmware tables. They are not measurements, and nothing of this board's layout data is confirmed on hardware. |

The firmware side of every table (key ids, scan positions, LED numbers, default key codes) comes from the board tables in `boards/<board>/tables/`, which for both boards match the board's stock firmware 2.14.1.

Machine-readable versions of the tables below, one directory per board (reference data, not compiled):

- Wooting 80HE: [`key_positions_ansi.csv`](../boards/wooting_80he/layout/key_positions_ansi.csv), [`key_positions_iso.csv`](../boards/wooting_80he/layout/key_positions_iso.csv), [`key_positions_jis.csv`](../boards/wooting_80he/layout/key_positions_jis.csv), [`leds.csv`](../boards/wooting_80he/layout/leds.csv) in `boards/wooting_80he/layout/`
- Wooting 60HE+: [`key_positions_ansi.csv`](../boards/wooting_60he_plus/layout/key_positions_ansi.csv), [`key_positions_iso.csv`](../boards/wooting_60he_plus/layout/key_positions_iso.csv), [`leds.csv`](../boards/wooting_60he_plus/layout/leds.csv) in `boards/wooting_60he_plus/layout/`

Related documents: [hardware](hardware.md) describes the electronics as the firmware uses them (sensor multiplexers, LED driver, pins). [extending-rgb](extending-rgb.md) describes the lighting model and how to add an effect.

## Coordinate system

All positions of one board use one frame for all its variants:

- Origin: the centre of the switch of the top-left key (Esc). On the 80HE this is the key of the function row (row 0), on the 60HE+ the key of the number row (row 1, the first row with keys).
- `x`: to the right, in millimetres.
- `y`: towards the user (down the keyboard), in millimetres.
- Key sizes are given in key units: 1u = 19.05 mm.

A position is the centre of the switch. The origin is the same point of the keyboard in all variants of a board, so coordinates of different variants can be compared directly. The positions are design values (80HE) or nominal grid values (60HE+).

## How positions relate to the firmware grid

The firmware does not know millimetres. It addresses every key and every key LED by a row and a column of a grid of `BOARD_KEY_ROWS` x `BOARD_KEY_COLS` cells: 6 x 17 on the 80HE, 6 x 14 on the 60HE+. Four numbers name a key:

| Number | Definition | Used for |
| --- | --- | --- |
| Key id | `row << 5 \| col` (`KEYS_KEY_ID` in `src/keys/keys_types.h`), one byte | The firmware's name of a key: content of the tables `k_layout_scanpos_to_keyid_<variant>`, key events, stored settings, host protocol |
| Cell index | `row * BOARD_KEY_COLS + col`: 0..101 on the 80HE, 0..83 on the 60HE+ | Index of the cell in a table of rows x columns entries (`g_layout_keyid_to_scanpos`, `g_rgb_key_to_led`). This is the column `cell` of the key CSV files. |
| Scan position | multiplexer address (0..15) + 16 x multiplexer index: 0..95 on the 80HE, 0..63 on the 60HE+ | Sensor channel of the key and index of its per-key record in `g_analog_key_states`. See [hardware](hardware.md#sensor-scan). |
| LED number | 80HE: 1..104, 105 means "no LED". 60HE+: 0..49 and 90..107, 108 means "no LED". | Output channel group of the LED driver. See [hardware](hardware.md#is31fl3741-led-driver). |

**Key id and cell index are two encodings of the same row and column, and they are different numbers.** On the 80HE the key at row 2, column 3 has the key id `2 << 5 | 3` = 67 (`0x43`) and the cell index `2 * 17 + 3` = 37; on the 60HE+ the same key id has the cell index `2 * 14 + 3` = 31. Everything in the firmware and in the host protocol that is called a key id is the first form. To convert a cell index: `row = cell / BOARD_KEY_COLS`, `col = cell % BOARD_KEY_COLS`, key id = `row << 5 | col`; in the other direction `row = id >> 5`, `col = id & 0x1F`. The reference tables below list both numbers for every key.

The tables that connect them, one set per variant, are collected in the board table `k_layout_variants` (`boards/<board>/tables/layout_tables.c`). At start-up `layout_select_variant` (`src/layout/layout_variant.c`) takes the entry of `g_layout_physical_variant` (0 ANSI, 1 ISO, 2 JIS); the 80HE reads the variant from the factory device information, the 60HE+ from bit 0 of a user option byte of its microcontroller ([hardware](hardware.md)):

- `g_layout_keyid_to_scanpos[cell index]` gives the scan position of a key.
- `g_layout_scanpos_to_keyid[scan position]` gives the key id of the key at a sensor channel.
- `g_rgb_key_to_led[cell index]` gives the LED number of a cell, or the "no LED" value (`boards/<board>/tables/rgb_tables.c`).
- The key maps use the same row and column (21 columns per row are stored on both boards, `BOARD_KEYMAP_COLS`; the columns from `BOARD_KEY_COLS` on are unused).

Grid rows are the physical rows, top to bottom. Grid columns follow the keys from left to right, but they are not a ruler:

- Columns are skipped. On the 80HE row 0 has no column 9, row 1 (ANSI, ISO) has no column 13, row 5 (ANSI, ISO) uses columns 0, 1, 2, 6, 10, 11, 12, 14, 15, 16. On the 60HE+ row 5 uses columns 0, 1, 2, 6, 10, 11, 12, 13.
- The same column sits at different x in different rows. On the 80HE column 14 runs from x = 250.03 mm (ANSI Right Shift) to x = 271.46 mm (Left arrow); column 1 is at x = 23.81 mm in row 0 (F1), 19.05 mm in row 1, 28.57 mm in row 2 and 33.34 mm in row 3.
- On the 80HE rows are not evenly spaced: row 0 is 23.81 mm above row 1, all other rows are 19.05 mm apart. On the 60HE+ row 0 carries no key (`BOARD_KEY_FIRST_ROW` = 1).
- Wide keys count as one cell. The space bar is cell (5, 6) on both boards, although it covers the width of more than six keys.

Some cells of row 5 have an LED but no switch: the LEDs under the space bar on both sides of its switch. They are listed as "LED-only cells" in each variant section.

The pictures at the start of each variant section show which column number sits where.

### What this means for lighting effects

The built-in effects compute with grid coordinates. The clearest case is the pair of tables built by `rgb_init_polar_tables` (`src/rgb/rgb_effects_state.c`) and used by the polar gradient effect: for every cell they store the distance and the angle to the point (`BOARD_KEY_ROWS` / 2, `BOARD_KEY_COLS` / 2) = (row 3.0, column 8.5) on the 80HE and (row 3.0, column 7.0) on the 60HE+, with one row step and one column step counted as the same length. On the 80HE this has three consequences:

- The reference point is not the middle of the keyboard. Row 3 is at y = 61.91 mm and column 8.5 of that row lies between x = 166.69 and 185.74 mm, so the point is near x = 176 mm, y = 62 mm. The middle of the key field (key cap edges from x = -9.53 to 319.09 mm and y = -9.53 to 109.54 mm) is at x = 154.8 mm, y = 50.0 mm, about 21 mm to the left and 12 mm further back.
- Shapes are sheared. Keys of one column are up to 21 mm apart in x, so a straight vertical line in the grid is a zigzag on the desk, and a circle in the grid is not a circle.
- The function row is treated as if it were 19.05 mm from the number row. It is 23.81 mm away.

On the 60HE+ the point is the key J of the nominal grid (row 3, column 7), and row 0 of the grid, which has no keys, still counts as one row step.

An effect that should look geometrically exact (a true circle, a wave with a constant speed in millimetres, a gradient that continues across the light bar) can use the positions of this document instead. A possible way:

1. Generate a constant table from the CSV file of your board and variant, for example the LED position of every cell in tenths of a millimetre. On the 80HE the LED window of a key is at the switch position plus (0, -4.94) mm, for the two-row Enter at (-4.94, 0) mm; the LED-only cells are in `leds.csv`. For the 60HE+ only nominal key positions exist.
2. In the effect, loop over rows and columns as the built-in effects do, look up the LED number in `g_rgb_key_to_led`, skip the "no LED" value (`RGB_LED_NONE`), and compute the colour from the millimetre position.
3. For the light bar of the 80HE, use the positions of the `lightbar` lines of [`leds.csv`](../boards/wooting_80he/layout/leds.csv) in the same frame. Read the note on the order of these LEDs in the [light-bar section](#light-bar) first.

A sketch of the table generator, run from the root of the repository (the board, the variant and the output name are examples; `BOARD_KEY_ROWS` and `BOARD_KEY_COLS` of the board give the loop bounds):

```python
import csv

board, rows, cols = 'wooting_80he', 6, 17
cells = {(int(r['row']), int(r['col'])): r
         for r in csv.DictReader(open('boards/%s/layout/key_positions_iso.csv' % board))}
print('static const int16_t k_led_xy_tenth_mm[%d][%d][2] = {' % (rows, cols))
for row in range(rows):
    items = []
    for col in range(cols):
        r = cells.get((row, col))
        if r is None:
            items.append('{0, 0}')      # no switch here; LED-only cells must be added by hand
            continue
        turned = r['height_u'] == '2'   # the two-row Enter is turned by 90 degrees
        x = float(r['x_mm']) - (4.94 if turned else 0.0)
        y = float(r['y_mm']) - (0.0 if turned else 4.94)
        items.append('{%d, %d}' % (round(x * 10), round(y * 10)))
    print('    { ' + ', '.join(items) + ' },')
print('};')
```

A firmware that should run on more than one variant needs one position table per variant, selected the same way as the layout tables in `layout_select_variant`. How to add an effect is described in [extending-rgb](extending-rgb.md).

## Format of the CSV files

The files in `boards/<board>/layout/` are comma-separated text with one header line. Coordinates use the frame of the board described under [Coordinate system](#coordinate-system) and have three decimals.

`key_positions_<variant>.csv`: one line per key switch, ordered by row and column. LED-only cells have no line.

| Column | Content |
| --- | --- |
| `row`, `col` | Grid cell of the key |
| `cell` | The cell index `row * BOARD_KEY_COLS + col`. This is not the firmware's key id `row << 5 \| col`; see [above](#how-positions-relate-to-the-firmware-grid). |
| `x_mm`, `y_mm` | Centre of the switch |
| `width_u`, `height_u` | Size of the key cap in key units (1u = 19.05 mm). `height_u` is 1 except for the two-row Enter of the ISO and JIS variants (2). |
| `led_id` | LED number of the key's LED |
| `default_keycode_layer0` | Key code of the key in the factory key map of layer 0, hexadecimal. This is the firmware's own key code (index into `k_layout_keycode_actions`), not a USB HID usage. |
| `position_source` | Where the position comes from, see below |

`leds.csv`: one line per LED and variant.

| Column | Content |
| --- | --- |
| `kind` | `key` (an LED under a key or an LED-only cell) or `lightbar` |
| `led_id` | LED number |
| `variant` | `ansi`, `iso`, `jis`, or `all` for the light bar |
| `row`, `col` | Grid cell of a key LED; empty for the light bar |
| `x_mm`, `y_mm` | Position; empty when unknown |
| `position_source` | Where the position comes from, see below |

Values of `position_source`:

| Value | Board | Meaning |
| --- | --- | --- |
| `design_files` | 80HE | Measured in the mechanical design files: switch centre (key files) or LED window (LED-only cells in `leds.csv`) |
| `under_key_design_files` | 80HE | (`leds.csv`) The switch centre of the key above the LED, from the design files; the LED window is 4.94 mm behind it (two-row Enter: 4.94 mm to the left) |
| `design_files_order_assumed` | 80HE | (`leds.csv`, light bar) The ten positions are from the design files; that light-bar position 0 is the left end is an assumption, see [Light bar](#light-bar) |
| `nominal` | 60HE+ | (key files) Row, column, LED number and key code are those of the tables; position and size are those of the usual 60 % key grid, not measurements |
| `assumed_under_key` | 60HE+ | (`leds.csv`) The LED is listed with the nominal position of its key |
| `unknown` | 60HE+ | (`leds.csv`) The LED belongs to a matrix cell without a key (under the space bar); no position is given |

## Reference tables: how to read them

In the pictures, one text line is one key row, four characters are one key unit, and the number inside a key is its grid column. The tables list, for every switch position: the cell index, the grid cell, the key id, the position of the switch centre, the key width, the key of the factory key map of layer 0 with its key code, the LED number, and the scan position (in brackets the ADC input, which is the multiplexer, and the multiplexer channel).

The names in the "Default key" column are the names of the USB HID usage that the key code produces (`k_layout_keycode_actions` in `src/tables/layout_keycode_tables.c`). They are not the legends printed on the key caps: "International1" to "International5" are the five additional keys of a Japanese keyboard, "Non-US \\" is the additional key of an ISO keyboard, "Application" is the menu key, and "action N" is a firmware function. The name in brackets is the end of the name of its handler in `src/layout/layout_key_actions.c`: momentary_layer1 is `layout_key_action_momentary_layer1`, the Fn key.

All keys are 1u high except the two-row Enter of ISO and JIS (2u). That key is L-shaped: the lower part is 1.25u wide and centred on the switch, the part in the row above is 1.5u wide and extends to the left (shown as `14v` or `13v` in the pictures). The tables give 1.25u.

## Wooting 80HE

### Overview

| | ANSI | ISO | JIS |
| --- | ---: | ---: | ---: |
| Switch positions | 84 | 85 | 88 |
| Switch positions per row, rows 0 to 5 | 16, 16, 16, 13, 13, 10 | 16, 16, 15, 14, 14, 10 | 16, 17, 15, 14, 14, 12 |
| Key LEDs (windows in the circuit board) | 88 | 89 | 90 |
| Unused sensor channels (of 96) | 12 | 11 | 8 |
| Stabilized keys | 5 | 4 | 3 |
| Space bar width | 6.25u | 6.25u | 4.5u |
| LED windows under the space bar | 5 | 5 | 3 |
| Light-bar LEDs | 10 | 10 | 10 |

Dimensions that are the same for the three variants:

| Item | Value |
| --- | --- |
| Switch plate | 330.76 x 121.21 mm, from x = -10.60 to 320.16 and y = -10.60 to 110.61; sheet 1.50 mm thick; switch cutouts 14.0 x 14.0 mm |
| Main circuit board | 328.26 x 119.00 x 1.60 mm, from x = -8.22 to 320.04 and y = -8.99 to 110.01; its top face is 5.00 mm below the top face of the plate |
| Circuit board outline | Rectangle with 2 mm chamfered corners; the right edge has a tab 2.26 mm deep between y = 56.51 and 67.21 (next to the light bar) |
| Key rows | y = 0.00, 23.81, 42.86, 61.91, 80.96, 100.01 mm: row 0 is 1.25u above row 1, the other rows are 1u apart |
| Switch footprint on the circuit board | Two 1.75 mm holes, 5.08 mm left and right of the switch centre |
| Key LED window | 3.7 x 3.4 mm slot, centred 4.94 mm behind the switch centre (towards the rear of the keyboard: `y - 4.94`) |
| Gap between key caps and top case | 0.84 to 0.92 mm on each side |

The two-row Enter key of the ISO and JIS variants is turned by 90 degrees: its two holes are in front of and behind the switch centre, and its LED window is 4.94 mm to the left (`x - 4.94`).

The three variants use three different circuit boards and three different switch plates. The top case, the light bar and the USB board are the same parts.

### Reference tables

#### ANSI

```text
row 0  [0 ] [1 ][2 ][3 ][4 ] [5 ][6 ][7 ][8 ] [10][11][12][13] [14] [15][16]
row 1  [0 ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][  14  ] [15][16]
row 2  [ 0  ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][ 14 ] [15][16]
row 3  [  0  ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][   14  ]
row 4  [   0   ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][    14   ] [15]
row 5  [ 0 ][ 1 ][ 2 ][           6           ][ 10][ 11][ 12]  [14][15][16]
```

Row 0 (y = 0.00 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 0 | 0, 0 | `0x00` | 0.00 | 0.00 | 1 | Esc (`0x26`) | 1 | `0x0C` (0, 12) |
| 1 | 0, 1 | `0x01` | 23.81 | 0.00 | 1 | F1 (`0x37`) | 14 | `0x0A` (0, 10) |
| 2 | 0, 2 | `0x02` | 42.86 | 0.00 | 1 | F2 (`0x38`) | 27 | `0x08` (0, 8) |
| 3 | 0, 3 | `0x03` | 61.91 | 0.00 | 1 | F3 (`0x39`) | 40 | `0x04` (0, 4) |
| 4 | 0, 4 | `0x04` | 80.96 | 0.00 | 1 | F4 (`0x3A`) | 53 | `0x01` (0, 1) |
| 5 | 0, 5 | `0x05` | 104.77 | 0.00 | 1 | F5 (`0x3B`) | 66 | `0x2B` (2, 11) |
| 6 | 0, 6 | `0x06` | 123.82 | 0.00 | 1 | F6 (`0x3C`) | 79 | `0x2A` (2, 10) |
| 7 | 0, 7 | `0x07` | 142.87 | 0.00 | 1 | F7 (`0x3D`) | 92 | `0x29` (2, 9) |
| 8 | 0, 8 | `0x08` | 161.92 | 0.00 | 1 | F8 (`0x3E`) | 7 | `0x21` (2, 1) |
| 10 | 0, 10 | `0x0A` | 185.74 | 0.00 | 1 | F9 (`0x3F`) | 20 | `0x20` (2, 0) |
| 11 | 0, 11 | `0x0B` | 204.79 | 0.00 | 1 | F10 (`0x40`) | 33 | `0x4A` (4, 10) |
| 12 | 0, 12 | `0x0C` | 223.84 | 0.00 | 1 | F11 (`0x41`) | 46 | `0x49` (4, 9) |
| 13 | 0, 13 | `0x0D` | 242.89 | 0.00 | 1 | F12 (`0x42`) | 59 | `0x48` (4, 8) |
| 14 | 0, 14 | `0x0E` | 266.70 | 0.00 | 1 | action 8 (toggle_mode_profile) (`0xE6`) | 72 | `0x47` (4, 7) |
| 15 | 0, 15 | `0x0F` | 290.51 | 0.00 | 1 | Print Screen (`0x43`) | 85 | `0x46` (4, 6) |
| 16 | 0, 16 | `0x10` | 309.56 | 0.00 | 1 | Pause (`0x45`) | 98 | `0x45` (4, 5) |

Row 1 (y = 23.81 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 17 | 1, 0 | `0x20` | 0.00 | 23.81 | 1 | Grave (`0x32`) | 2 | `0x0D` (0, 13) |
| 18 | 1, 1 | `0x21` | 19.05 | 23.81 | 1 | 1 (`0x1B`) | 15 | `0x0B` (0, 11) |
| 19 | 1, 2 | `0x22` | 38.10 | 23.81 | 1 | 2 (`0x1C`) | 28 | `0x07` (0, 7) |
| 20 | 1, 3 | `0x23` | 57.15 | 23.81 | 1 | 3 (`0x1D`) | 41 | `0x05` (0, 5) |
| 21 | 1, 4 | `0x24` | 76.20 | 23.81 | 1 | 4 (`0x1E`) | 54 | `0x02` (0, 2) |
| 22 | 1, 5 | `0x25` | 95.25 | 23.81 | 1 | 5 (`0x1F`) | 67 | `0x2C` (2, 12) |
| 23 | 1, 6 | `0x26` | 114.30 | 23.81 | 1 | 6 (`0x20`) | 80 | `0x28` (2, 8) |
| 24 | 1, 7 | `0x27` | 133.35 | 23.81 | 1 | 7 (`0x21`) | 93 | `0x26` (2, 6) |
| 25 | 1, 8 | `0x28` | 152.40 | 23.81 | 1 | 8 (`0x22`) | 8 | `0x24` (2, 4) |
| 26 | 1, 9 | `0x29` | 171.45 | 23.81 | 1 | 9 (`0x23`) | 21 | `0x22` (2, 2) |
| 27 | 1, 10 | `0x2A` | 190.50 | 23.81 | 1 | 0 (`0x24`) | 34 | `0x4B` (4, 11) |
| 28 | 1, 11 | `0x2B` | 209.55 | 23.81 | 1 | - (`0x2A`) | 47 | `0x4C` (4, 12) |
| 29 | 1, 12 | `0x2C` | 228.60 | 23.81 | 1 | = (`0x2B`) | 60 | `0x4D` (4, 13) |
| 31 | 1, 14 | `0x2E` | 257.17 | 23.81 | 2 | Backspace (`0x27`) | 73 | `0x40` (4, 0) |
| 32 | 1, 15 | `0x2F` | 290.51 | 23.81 | 1 | Insert (`0x46`) | 86 | `0x41` (4, 1) |
| 33 | 1, 16 | `0x30` | 309.56 | 23.81 | 1 | Page Up (`0x48`) | 99 | `0x44` (4, 4) |

Row 2 (y = 42.86 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 34 | 2, 0 | `0x40` | 4.76 | 42.86 | 1.5 | Tab (`0x28`) | 3 | `0x19` (1, 9) |
| 35 | 2, 1 | `0x41` | 28.57 | 42.86 | 1 | Q (`0x11`) | 16 | `0x09` (0, 9) |
| 36 | 2, 2 | `0x42` | 47.62 | 42.86 | 1 | W (`0x17`) | 29 | `0x06` (0, 6) |
| 37 | 2, 3 | `0x43` | 66.67 | 42.86 | 1 | E (`0x05`) | 42 | `0x03` (0, 3) |
| 38 | 2, 4 | `0x44` | 85.72 | 42.86 | 1 | R (`0x12`) | 55 | `0x00` (0, 0) |
| 39 | 2, 5 | `0x45` | 104.77 | 42.86 | 1 | T (`0x14`) | 68 | `0x2D` (2, 13) |
| 40 | 2, 6 | `0x46` | 123.82 | 42.86 | 1 | Y (`0x19`) | 81 | `0x27` (2, 7) |
| 41 | 2, 7 | `0x47` | 142.87 | 42.86 | 1 | U (`0x15`) | 94 | `0x25` (2, 5) |
| 42 | 2, 8 | `0x48` | 161.92 | 42.86 | 1 | I (`0x09`) | 9 | `0x23` (2, 3) |
| 43 | 2, 9 | `0x49` | 180.98 | 42.86 | 1 | O (`0x0F`) | 22 | `0x36` (3, 6) |
| 44 | 2, 10 | `0x4A` | 200.03 | 42.86 | 1 | P (`0x10`) | 35 | `0x5B` (5, 11) |
| 45 | 2, 11 | `0x4B` | 219.08 | 42.86 | 1 | [ (`0x2C`) | 48 | `0x5A` (5, 10) |
| 46 | 2, 12 | `0x4C` | 238.13 | 42.86 | 1 | ] (`0x2D`) | 61 | `0x59` (5, 9) |
| 48 | 2, 14 | `0x4E` | 261.94 | 42.86 | 1.5 | \ (`0x2E`) | 62 | `0x57` (5, 7) |
| 49 | 2, 15 | `0x4F` | 290.51 | 42.86 | 1 | Delete (`0x49`) | 87 | `0x42` (4, 2) |
| 50 | 2, 16 | `0x50` | 309.56 | 42.86 | 1 | Page Down (`0x4B`) | 100 | `0x43` (4, 3) |

Row 3 (y = 61.91 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 51 | 3, 0 | `0x60` | 7.14 | 61.91 | 1.75 | Caps Lock (`0x36`) | 4 | `0x1A` (1, 10) |
| 52 | 3, 1 | `0x61` | 33.34 | 61.91 | 1 | A (`0x01`) | 17 | `0x18` (1, 8) |
| 53 | 3, 2 | `0x62` | 52.39 | 61.91 | 1 | S (`0x13`) | 30 | `0x17` (1, 7) |
| 54 | 3, 3 | `0x63` | 71.44 | 61.91 | 1 | D (`0x04`) | 43 | `0x16` (1, 6) |
| 55 | 3, 4 | `0x64` | 90.49 | 61.91 | 1 | F (`0x06`) | 56 | `0x15` (1, 5) |
| 56 | 3, 5 | `0x65` | 109.54 | 61.91 | 1 | G (`0x07`) | 69 | `0x3A` (3, 10) |
| 57 | 3, 6 | `0x66` | 128.59 | 61.91 | 1 | H (`0x08`) | 82 | `0x39` (3, 9) |
| 58 | 3, 7 | `0x67` | 147.64 | 61.91 | 1 | J (`0x0A`) | 95 | `0x38` (3, 8) |
| 59 | 3, 8 | `0x68` | 166.69 | 61.91 | 1 | K (`0x0B`) | 10 | `0x37` (3, 7) |
| 60 | 3, 9 | `0x69` | 185.74 | 61.91 | 1 | L (`0x0C`) | 23 | `0x35` (3, 5) |
| 61 | 3, 10 | `0x6A` | 204.79 | 61.91 | 1 | ; (`0x30`) | 36 | `0x5C` (5, 12) |
| 62 | 3, 11 | `0x6B` | 223.84 | 61.91 | 1 | ' (`0x31`) | 49 | `0x58` (5, 8) |
| 65 | 3, 14 | `0x6E` | 254.79 | 61.91 | 2.25 | Enter (`0x25`) | 75 | `0x56` (5, 6) |

Row 4 (y = 80.96 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 68 | 4, 0 | `0x80` | 11.91 | 80.96 | 2.25 | Left Shift (`0x64`) | 5 | `0x1B` (1, 11) |
| 70 | 4, 2 | `0x82` | 42.86 | 80.96 | 1 | Z (`0x1A`) | 31 | `0x10` (1, 0) |
| 71 | 4, 3 | `0x83` | 61.91 | 80.96 | 1 | X (`0x18`) | 44 | `0x12` (1, 2) |
| 72 | 4, 4 | `0x84` | 80.96 | 80.96 | 1 | C (`0x03`) | 57 | `0x13` (1, 3) |
| 73 | 4, 5 | `0x85` | 100.01 | 80.96 | 1 | V (`0x16`) | 70 | `0x14` (1, 4) |
| 74 | 4, 6 | `0x86` | 119.06 | 80.96 | 1 | B (`0x02`) | 83 | `0x3B` (3, 11) |
| 75 | 4, 7 | `0x87` | 138.11 | 80.96 | 1 | N (`0x0E`) | 96 | `0x3D` (3, 13) |
| 76 | 4, 8 | `0x88` | 157.16 | 80.96 | 1 | M (`0x0D`) | 11 | `0x30` (3, 0) |
| 77 | 4, 9 | `0x89` | 176.21 | 80.96 | 1 | , (`0x33`) | 24 | `0x31` (3, 1) |
| 78 | 4, 10 | `0x8A` | 195.26 | 80.96 | 1 | . (`0x34`) | 37 | `0x32` (3, 2) |
| 79 | 4, 11 | `0x8B` | 214.31 | 80.96 | 1 | / (`0x35`) | 50 | `0x5D` (5, 13) |
| 82 | 4, 14 | `0x8E` | 250.03 | 80.96 | 2.75 | Right Shift (`0x68`) | 63 | `0x51` (5, 1) |
| 83 | 4, 15 | `0x8F` | 290.51 | 80.96 | 1 | Up (`0x4F`) | 76 | `0x55` (5, 5) |

Row 5 (y = 100.01 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 85 | 5, 0 | `0xA0` | 2.38 | 100.01 | 1.25 | Left Ctrl (`0x63`) | 6 | `0x1C` (1, 12) |
| 86 | 5, 1 | `0xA1` | 26.19 | 100.01 | 1.25 | Left GUI (`0x66`) | 19 | `0x1E` (1, 14) |
| 87 | 5, 2 | `0xA2` | 50.01 | 100.01 | 1.25 | Left Alt (`0x65`) | 32 | `0x11` (1, 1) |
| 91 | 5, 6 | `0xA6` | 121.44 | 100.01 | 6.25 | Space (`0x29`) | 71 | `0x3C` (3, 12) |
| 95 | 5, 10 | `0xAA` | 192.88 | 100.01 | 1.25 | Right Alt (`0x69`) | 12 | `0x33` (3, 3) |
| 96 | 5, 11 | `0xAB` | 216.69 | 100.01 | 1.25 | Right Ctrl (`0x67`) | 25 | `0x34` (3, 4) |
| 97 | 5, 12 | `0xAC` | 240.51 | 100.01 | 1.25 | action 9 (momentary_layer1) (`0xE7`) | 38 | `0x50` (5, 0) |
| 99 | 5, 14 | `0xAE` | 271.46 | 100.01 | 1 | Left (`0x4D`) | 13 | `0x52` (5, 2) |
| 100 | 5, 15 | `0xAF` | 290.51 | 100.01 | 1 | Down (`0x4E`) | 26 | `0x53` (5, 3) |
| 101 | 5, 16 | `0xB0` | 309.56 | 100.01 | 1 | Right (`0x4C`) | 39 | `0x54` (5, 4) |

LED-only cells:

| Row, col | LED number | x mm | y mm |
| :---: | ---: | ---: | ---: |
| 5, 4 | 45 | 82.04 | 100.01 |
| 5, 5 | 58 | 101.74 | 100.01 |
| 5, 7 | 84 | 141.14 | 100.01 |
| 5, 8 | 97 | 160.84 | 100.01 |

Stabilizers:

| Cell | Key | Direction | Wire spacing mm | Part name in the design files |
| :---: | --- | --- | ---: | --- |
| 1, 14 | Backspace | along the row | 23.80 | `STABILIZER-2P_2` |
| 3, 14 | Enter | along the row | 23.80 | `STABILIZER-2P_2` |
| 4, 0 | Left Shift | along the row | 23.80 | `STABILIZER-2P_2` |
| 4, 14 | Right Shift | along the row | 23.80 | `STABILIZER-2P_2` |
| 5, 6 | Space | along the row | 100.05 | `STABILIZER-6P25_3` |

#### ISO

```text
row 0  [0 ] [1 ][2 ][3 ][4 ] [5 ][6 ][7 ][8 ] [10][11][12][13] [14] [15][16]
row 1  [0 ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][  14  ] [15][16]
row 2  [ 0  ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][14v ] [15][16]
row 3  [  0  ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][ 14]
row 4  [ 0 ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][    14   ] [15]
row 5  [ 0 ][ 1 ][ 2 ][           6           ][ 10][ 11][ 12]  [14][15][16]
```

Row 0 (y = 0.00 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 0 | 0, 0 | `0x00` | 0.00 | 0.00 | 1 | Esc (`0x26`) | 1 | `0x0C` (0, 12) |
| 1 | 0, 1 | `0x01` | 23.81 | 0.00 | 1 | F1 (`0x37`) | 14 | `0x0A` (0, 10) |
| 2 | 0, 2 | `0x02` | 42.86 | 0.00 | 1 | F2 (`0x38`) | 27 | `0x08` (0, 8) |
| 3 | 0, 3 | `0x03` | 61.91 | 0.00 | 1 | F3 (`0x39`) | 40 | `0x04` (0, 4) |
| 4 | 0, 4 | `0x04` | 80.96 | 0.00 | 1 | F4 (`0x3A`) | 53 | `0x01` (0, 1) |
| 5 | 0, 5 | `0x05` | 104.77 | 0.00 | 1 | F5 (`0x3B`) | 66 | `0x2B` (2, 11) |
| 6 | 0, 6 | `0x06` | 123.82 | 0.00 | 1 | F6 (`0x3C`) | 79 | `0x2A` (2, 10) |
| 7 | 0, 7 | `0x07` | 142.87 | 0.00 | 1 | F7 (`0x3D`) | 92 | `0x29` (2, 9) |
| 8 | 0, 8 | `0x08` | 161.92 | 0.00 | 1 | F8 (`0x3E`) | 7 | `0x21` (2, 1) |
| 10 | 0, 10 | `0x0A` | 185.74 | 0.00 | 1 | F9 (`0x3F`) | 20 | `0x20` (2, 0) |
| 11 | 0, 11 | `0x0B` | 204.79 | 0.00 | 1 | F10 (`0x40`) | 33 | `0x4A` (4, 10) |
| 12 | 0, 12 | `0x0C` | 223.84 | 0.00 | 1 | F11 (`0x41`) | 46 | `0x49` (4, 9) |
| 13 | 0, 13 | `0x0D` | 242.89 | 0.00 | 1 | F12 (`0x42`) | 59 | `0x48` (4, 8) |
| 14 | 0, 14 | `0x0E` | 266.70 | 0.00 | 1 | action 8 (toggle_mode_profile) (`0xE6`) | 72 | `0x47` (4, 7) |
| 15 | 0, 15 | `0x0F` | 290.51 | 0.00 | 1 | Print Screen (`0x43`) | 85 | `0x46` (4, 6) |
| 16 | 0, 16 | `0x10` | 309.56 | 0.00 | 1 | Pause (`0x45`) | 98 | `0x45` (4, 5) |

Row 1 (y = 23.81 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 17 | 1, 0 | `0x20` | 0.00 | 23.81 | 1 | Grave (`0x32`) | 2 | `0x0D` (0, 13) |
| 18 | 1, 1 | `0x21` | 19.05 | 23.81 | 1 | 1 (`0x1B`) | 15 | `0x0B` (0, 11) |
| 19 | 1, 2 | `0x22` | 38.10 | 23.81 | 1 | 2 (`0x1C`) | 28 | `0x07` (0, 7) |
| 20 | 1, 3 | `0x23` | 57.15 | 23.81 | 1 | 3 (`0x1D`) | 41 | `0x05` (0, 5) |
| 21 | 1, 4 | `0x24` | 76.20 | 23.81 | 1 | 4 (`0x1E`) | 54 | `0x02` (0, 2) |
| 22 | 1, 5 | `0x25` | 95.25 | 23.81 | 1 | 5 (`0x1F`) | 67 | `0x2C` (2, 12) |
| 23 | 1, 6 | `0x26` | 114.30 | 23.81 | 1 | 6 (`0x20`) | 80 | `0x28` (2, 8) |
| 24 | 1, 7 | `0x27` | 133.35 | 23.81 | 1 | 7 (`0x21`) | 93 | `0x26` (2, 6) |
| 25 | 1, 8 | `0x28` | 152.40 | 23.81 | 1 | 8 (`0x22`) | 8 | `0x24` (2, 4) |
| 26 | 1, 9 | `0x29` | 171.45 | 23.81 | 1 | 9 (`0x23`) | 21 | `0x22` (2, 2) |
| 27 | 1, 10 | `0x2A` | 190.50 | 23.81 | 1 | 0 (`0x24`) | 34 | `0x4B` (4, 11) |
| 28 | 1, 11 | `0x2B` | 209.55 | 23.81 | 1 | - (`0x2A`) | 47 | `0x4C` (4, 12) |
| 29 | 1, 12 | `0x2C` | 228.60 | 23.81 | 1 | = (`0x2B`) | 60 | `0x4D` (4, 13) |
| 31 | 1, 14 | `0x2E` | 257.17 | 23.81 | 2 | Backspace (`0x27`) | 73 | `0x40` (4, 0) |
| 32 | 1, 15 | `0x2F` | 290.51 | 23.81 | 1 | Insert (`0x46`) | 86 | `0x41` (4, 1) |
| 33 | 1, 16 | `0x30` | 309.56 | 23.81 | 1 | Page Up (`0x48`) | 99 | `0x44` (4, 4) |

Row 2 (y = 42.86 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 34 | 2, 0 | `0x40` | 4.76 | 42.86 | 1.5 | Tab (`0x28`) | 3 | `0x19` (1, 9) |
| 35 | 2, 1 | `0x41` | 28.57 | 42.86 | 1 | Q (`0x11`) | 16 | `0x09` (0, 9) |
| 36 | 2, 2 | `0x42` | 47.62 | 42.86 | 1 | W (`0x17`) | 29 | `0x06` (0, 6) |
| 37 | 2, 3 | `0x43` | 66.67 | 42.86 | 1 | E (`0x05`) | 42 | `0x03` (0, 3) |
| 38 | 2, 4 | `0x44` | 85.72 | 42.86 | 1 | R (`0x12`) | 55 | `0x00` (0, 0) |
| 39 | 2, 5 | `0x45` | 104.77 | 42.86 | 1 | T (`0x14`) | 68 | `0x2D` (2, 13) |
| 40 | 2, 6 | `0x46` | 123.82 | 42.86 | 1 | Y (`0x19`) | 81 | `0x27` (2, 7) |
| 41 | 2, 7 | `0x47` | 142.87 | 42.86 | 1 | U (`0x15`) | 94 | `0x25` (2, 5) |
| 42 | 2, 8 | `0x48` | 161.92 | 42.86 | 1 | I (`0x09`) | 9 | `0x23` (2, 3) |
| 43 | 2, 9 | `0x49` | 180.98 | 42.86 | 1 | O (`0x0F`) | 22 | `0x36` (3, 6) |
| 44 | 2, 10 | `0x4A` | 200.03 | 42.86 | 1 | P (`0x10`) | 35 | `0x5B` (5, 11) |
| 45 | 2, 11 | `0x4B` | 219.08 | 42.86 | 1 | [ (`0x2C`) | 48 | `0x5A` (5, 10) |
| 46 | 2, 12 | `0x4C` | 238.13 | 42.86 | 1 | ] (`0x2D`) | 61 | `0x59` (5, 9) |
| 49 | 2, 15 | `0x4F` | 290.51 | 42.86 | 1 | Delete (`0x49`) | 87 | `0x42` (4, 2) |
| 50 | 2, 16 | `0x50` | 309.56 | 42.86 | 1 | Page Down (`0x4B`) | 100 | `0x43` (4, 3) |

Row 3 (y = 61.91 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 51 | 3, 0 | `0x60` | 7.14 | 61.91 | 1.75 | Caps Lock (`0x36`) | 4 | `0x1A` (1, 10) |
| 52 | 3, 1 | `0x61` | 33.34 | 61.91 | 1 | A (`0x01`) | 17 | `0x18` (1, 8) |
| 53 | 3, 2 | `0x62` | 52.39 | 61.91 | 1 | S (`0x13`) | 30 | `0x17` (1, 7) |
| 54 | 3, 3 | `0x63` | 71.44 | 61.91 | 1 | D (`0x04`) | 43 | `0x16` (1, 6) |
| 55 | 3, 4 | `0x64` | 90.49 | 61.91 | 1 | F (`0x06`) | 56 | `0x15` (1, 5) |
| 56 | 3, 5 | `0x65` | 109.54 | 61.91 | 1 | G (`0x07`) | 69 | `0x3A` (3, 10) |
| 57 | 3, 6 | `0x66` | 128.59 | 61.91 | 1 | H (`0x08`) | 82 | `0x39` (3, 9) |
| 58 | 3, 7 | `0x67` | 147.64 | 61.91 | 1 | J (`0x0A`) | 95 | `0x38` (3, 8) |
| 59 | 3, 8 | `0x68` | 166.69 | 61.91 | 1 | K (`0x0B`) | 10 | `0x37` (3, 7) |
| 60 | 3, 9 | `0x69` | 185.74 | 61.91 | 1 | L (`0x0C`) | 23 | `0x35` (3, 5) |
| 61 | 3, 10 | `0x6A` | 204.79 | 61.91 | 1 | ; (`0x30`) | 36 | `0x5C` (5, 12) |
| 62 | 3, 11 | `0x6B` | 223.84 | 61.91 | 1 | ' (`0x31`) | 49 | `0x58` (5, 8) |
| 63 | 3, 12 | `0x6C` | 242.89 | 61.91 | 1 | \ (`0x2E`) | 62 | `0x57` (5, 7) |
| 65 | 3, 14 | `0x6E` | 264.32 | 52.39 | 1.25 x 2u high | Enter (`0x25`) | 75 | `0x56` (5, 6) |

Row 4 (y = 80.96 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 68 | 4, 0 | `0x80` | 2.38 | 80.96 | 1.25 | Left Shift (`0x64`) | 5 | `0x1B` (1, 11) |
| 69 | 4, 1 | `0x81` | 23.81 | 80.96 | 1 | Non-US \ (`0x61`) | 18 | `0x1D` (1, 13) |
| 70 | 4, 2 | `0x82` | 42.86 | 80.96 | 1 | Z (`0x1A`) | 31 | `0x10` (1, 0) |
| 71 | 4, 3 | `0x83` | 61.91 | 80.96 | 1 | X (`0x18`) | 44 | `0x12` (1, 2) |
| 72 | 4, 4 | `0x84` | 80.96 | 80.96 | 1 | C (`0x03`) | 57 | `0x13` (1, 3) |
| 73 | 4, 5 | `0x85` | 100.01 | 80.96 | 1 | V (`0x16`) | 70 | `0x14` (1, 4) |
| 74 | 4, 6 | `0x86` | 119.06 | 80.96 | 1 | B (`0x02`) | 83 | `0x3B` (3, 11) |
| 75 | 4, 7 | `0x87` | 138.11 | 80.96 | 1 | N (`0x0E`) | 96 | `0x3D` (3, 13) |
| 76 | 4, 8 | `0x88` | 157.16 | 80.96 | 1 | M (`0x0D`) | 11 | `0x30` (3, 0) |
| 77 | 4, 9 | `0x89` | 176.21 | 80.96 | 1 | , (`0x33`) | 24 | `0x31` (3, 1) |
| 78 | 4, 10 | `0x8A` | 195.26 | 80.96 | 1 | . (`0x34`) | 37 | `0x32` (3, 2) |
| 79 | 4, 11 | `0x8B` | 214.31 | 80.96 | 1 | / (`0x35`) | 50 | `0x5D` (5, 13) |
| 82 | 4, 14 | `0x8E` | 250.03 | 80.96 | 2.75 | Right Shift (`0x68`) | 63 | `0x51` (5, 1) |
| 83 | 4, 15 | `0x8F` | 290.51 | 80.96 | 1 | Up (`0x4F`) | 76 | `0x55` (5, 5) |

Row 5 (y = 100.01 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 85 | 5, 0 | `0xA0` | 2.38 | 100.01 | 1.25 | Left Ctrl (`0x63`) | 6 | `0x1C` (1, 12) |
| 86 | 5, 1 | `0xA1` | 26.19 | 100.01 | 1.25 | Left GUI (`0x66`) | 19 | `0x1E` (1, 14) |
| 87 | 5, 2 | `0xA2` | 50.01 | 100.01 | 1.25 | Left Alt (`0x65`) | 32 | `0x11` (1, 1) |
| 91 | 5, 6 | `0xA6` | 121.44 | 100.01 | 6.25 | Space (`0x29`) | 71 | `0x3C` (3, 12) |
| 95 | 5, 10 | `0xAA` | 192.88 | 100.01 | 1.25 | Right Alt (`0x69`) | 12 | `0x33` (3, 3) |
| 96 | 5, 11 | `0xAB` | 216.69 | 100.01 | 1.25 | Right Ctrl (`0x67`) | 25 | `0x34` (3, 4) |
| 97 | 5, 12 | `0xAC` | 240.51 | 100.01 | 1.25 | action 9 (momentary_layer1) (`0xE7`) | 38 | `0x50` (5, 0) |
| 99 | 5, 14 | `0xAE` | 271.46 | 100.01 | 1 | Left (`0x4D`) | 13 | `0x52` (5, 2) |
| 100 | 5, 15 | `0xAF` | 290.51 | 100.01 | 1 | Down (`0x4E`) | 26 | `0x53` (5, 3) |
| 101 | 5, 16 | `0xB0` | 309.56 | 100.01 | 1 | Right (`0x4C`) | 39 | `0x54` (5, 4) |

LED-only cells:

| Row, col | LED number | x mm | y mm |
| :---: | ---: | ---: | ---: |
| 5, 4 | 45 | 82.04 | 100.01 |
| 5, 5 | 58 | 101.74 | 100.01 |
| 5, 7 | 84 | 141.14 | 100.01 |
| 5, 8 | 97 | 160.84 | 100.01 |

Stabilizers:

| Cell | Key | Direction | Wire spacing mm | Part name in the design files |
| :---: | --- | --- | ---: | --- |
| 1, 14 | Backspace | along the row | 23.80 | `STABILIZER-2P_2` |
| 3, 14 | Enter | across the rows | 23.80 | `STABILIZER-2P_2` |
| 4, 14 | Right Shift | along the row | 23.80 | `STABILIZER-2P_2` |
| 5, 6 | Space | along the row | 100.05 | `STABILIZER-6P25_3` |

#### JIS

```text
row 0  [0 ] [1 ][2 ][3 ][4 ] [5 ][6 ][7 ][8 ] [10][11][12][13] [14] [15][16]
row 1  [0 ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][13][14] [15][16]
row 2  [ 0  ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][14v ] [15][16]
row 3  [  0  ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][ 14]
row 4  [   0   ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][  14 ] [15]
row 5  [ 0 ][1 ][ 2 ][3 ][       6        ][ 9 ][ 10][ 11][12]  [14][15][16]
```

Row 0 (y = 0.00 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 0 | 0, 0 | `0x00` | 0.00 | 0.00 | 1 | Esc (`0x26`) | 1 | `0x0C` (0, 12) |
| 1 | 0, 1 | `0x01` | 23.81 | 0.00 | 1 | F1 (`0x37`) | 14 | `0x0A` (0, 10) |
| 2 | 0, 2 | `0x02` | 42.86 | 0.00 | 1 | F2 (`0x38`) | 27 | `0x08` (0, 8) |
| 3 | 0, 3 | `0x03` | 61.91 | 0.00 | 1 | F3 (`0x39`) | 40 | `0x04` (0, 4) |
| 4 | 0, 4 | `0x04` | 80.96 | 0.00 | 1 | F4 (`0x3A`) | 53 | `0x01` (0, 1) |
| 5 | 0, 5 | `0x05` | 104.77 | 0.00 | 1 | F5 (`0x3B`) | 66 | `0x2B` (2, 11) |
| 6 | 0, 6 | `0x06` | 123.82 | 0.00 | 1 | F6 (`0x3C`) | 79 | `0x2A` (2, 10) |
| 7 | 0, 7 | `0x07` | 142.87 | 0.00 | 1 | F7 (`0x3D`) | 92 | `0x29` (2, 9) |
| 8 | 0, 8 | `0x08` | 161.92 | 0.00 | 1 | F8 (`0x3E`) | 7 | `0x21` (2, 1) |
| 10 | 0, 10 | `0x0A` | 185.74 | 0.00 | 1 | F9 (`0x3F`) | 20 | `0x20` (2, 0) |
| 11 | 0, 11 | `0x0B` | 204.79 | 0.00 | 1 | F10 (`0x40`) | 33 | `0x4A` (4, 10) |
| 12 | 0, 12 | `0x0C` | 223.84 | 0.00 | 1 | F11 (`0x41`) | 46 | `0x49` (4, 9) |
| 13 | 0, 13 | `0x0D` | 242.89 | 0.00 | 1 | F12 (`0x42`) | 59 | `0x48` (4, 8) |
| 14 | 0, 14 | `0x0E` | 266.70 | 0.00 | 1 | action 8 (toggle_mode_profile) (`0xE6`) | 72 | `0x47` (4, 7) |
| 15 | 0, 15 | `0x0F` | 290.51 | 0.00 | 1 | Print Screen (`0x43`) | 85 | `0x46` (4, 6) |
| 16 | 0, 16 | `0x10` | 309.56 | 0.00 | 1 | Pause (`0x45`) | 98 | `0x45` (4, 5) |

Row 1 (y = 23.81 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 17 | 1, 0 | `0x20` | 0.00 | 23.81 | 1 | Grave (`0x32`) | 2 | `0x0D` (0, 13) |
| 18 | 1, 1 | `0x21` | 19.05 | 23.81 | 1 | 1 (`0x1B`) | 15 | `0x0B` (0, 11) |
| 19 | 1, 2 | `0x22` | 38.10 | 23.81 | 1 | 2 (`0x1C`) | 28 | `0x07` (0, 7) |
| 20 | 1, 3 | `0x23` | 57.15 | 23.81 | 1 | 3 (`0x1D`) | 41 | `0x05` (0, 5) |
| 21 | 1, 4 | `0x24` | 76.20 | 23.81 | 1 | 4 (`0x1E`) | 54 | `0x02` (0, 2) |
| 22 | 1, 5 | `0x25` | 95.25 | 23.81 | 1 | 5 (`0x1F`) | 67 | `0x2C` (2, 12) |
| 23 | 1, 6 | `0x26` | 114.30 | 23.81 | 1 | 6 (`0x20`) | 80 | `0x28` (2, 8) |
| 24 | 1, 7 | `0x27` | 133.35 | 23.81 | 1 | 7 (`0x21`) | 93 | `0x26` (2, 6) |
| 25 | 1, 8 | `0x28` | 152.40 | 23.81 | 1 | 8 (`0x22`) | 8 | `0x24` (2, 4) |
| 26 | 1, 9 | `0x29` | 171.45 | 23.81 | 1 | 9 (`0x23`) | 21 | `0x22` (2, 2) |
| 27 | 1, 10 | `0x2A` | 190.50 | 23.81 | 1 | 0 (`0x24`) | 34 | `0x4B` (4, 11) |
| 28 | 1, 11 | `0x2B` | 209.55 | 23.81 | 1 | - (`0x2A`) | 47 | `0x4C` (4, 12) |
| 29 | 1, 12 | `0x2C` | 228.60 | 23.81 | 1 | = (`0x2B`) | 60 | `0x4D` (4, 13) |
| 30 | 1, 13 | `0x2D` | 247.65 | 23.81 | 1 | International3 (`0xF4`) | 73 | `0x40` (4, 0) |
| 31 | 1, 14 | `0x2E` | 266.70 | 23.81 | 1 | Backspace (`0x27`) | 74 | `0x4E` (4, 14) |
| 32 | 1, 15 | `0x2F` | 290.51 | 23.81 | 1 | Insert (`0x46`) | 86 | `0x41` (4, 1) |
| 33 | 1, 16 | `0x30` | 309.56 | 23.81 | 1 | Page Up (`0x48`) | 99 | `0x44` (4, 4) |

Row 2 (y = 42.86 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 34 | 2, 0 | `0x40` | 4.76 | 42.86 | 1.5 | Tab (`0x28`) | 3 | `0x19` (1, 9) |
| 35 | 2, 1 | `0x41` | 28.57 | 42.86 | 1 | Q (`0x11`) | 16 | `0x09` (0, 9) |
| 36 | 2, 2 | `0x42` | 47.62 | 42.86 | 1 | W (`0x17`) | 29 | `0x06` (0, 6) |
| 37 | 2, 3 | `0x43` | 66.67 | 42.86 | 1 | E (`0x05`) | 42 | `0x03` (0, 3) |
| 38 | 2, 4 | `0x44` | 85.72 | 42.86 | 1 | R (`0x12`) | 55 | `0x00` (0, 0) |
| 39 | 2, 5 | `0x45` | 104.77 | 42.86 | 1 | T (`0x14`) | 68 | `0x2D` (2, 13) |
| 40 | 2, 6 | `0x46` | 123.82 | 42.86 | 1 | Y (`0x19`) | 81 | `0x27` (2, 7) |
| 41 | 2, 7 | `0x47` | 142.87 | 42.86 | 1 | U (`0x15`) | 94 | `0x25` (2, 5) |
| 42 | 2, 8 | `0x48` | 161.92 | 42.86 | 1 | I (`0x09`) | 9 | `0x23` (2, 3) |
| 43 | 2, 9 | `0x49` | 180.98 | 42.86 | 1 | O (`0x0F`) | 22 | `0x36` (3, 6) |
| 44 | 2, 10 | `0x4A` | 200.03 | 42.86 | 1 | P (`0x10`) | 35 | `0x5B` (5, 11) |
| 45 | 2, 11 | `0x4B` | 219.08 | 42.86 | 1 | [ (`0x2C`) | 48 | `0x5A` (5, 10) |
| 46 | 2, 12 | `0x4C` | 238.13 | 42.86 | 1 | ] (`0x2D`) | 61 | `0x59` (5, 9) |
| 49 | 2, 15 | `0x4F` | 290.51 | 42.86 | 1 | Delete (`0x49`) | 87 | `0x42` (4, 2) |
| 50 | 2, 16 | `0x50` | 309.56 | 42.86 | 1 | Page Down (`0x4B`) | 100 | `0x43` (4, 3) |

Row 3 (y = 61.91 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 51 | 3, 0 | `0x60` | 7.14 | 61.91 | 1.75 | Caps Lock (`0x36`) | 4 | `0x1A` (1, 10) |
| 52 | 3, 1 | `0x61` | 33.34 | 61.91 | 1 | A (`0x01`) | 17 | `0x18` (1, 8) |
| 53 | 3, 2 | `0x62` | 52.39 | 61.91 | 1 | S (`0x13`) | 30 | `0x17` (1, 7) |
| 54 | 3, 3 | `0x63` | 71.44 | 61.91 | 1 | D (`0x04`) | 43 | `0x16` (1, 6) |
| 55 | 3, 4 | `0x64` | 90.49 | 61.91 | 1 | F (`0x06`) | 56 | `0x15` (1, 5) |
| 56 | 3, 5 | `0x65` | 109.54 | 61.91 | 1 | G (`0x07`) | 69 | `0x3A` (3, 10) |
| 57 | 3, 6 | `0x66` | 128.59 | 61.91 | 1 | H (`0x08`) | 82 | `0x39` (3, 9) |
| 58 | 3, 7 | `0x67` | 147.64 | 61.91 | 1 | J (`0x0A`) | 95 | `0x38` (3, 8) |
| 59 | 3, 8 | `0x68` | 166.69 | 61.91 | 1 | K (`0x0B`) | 10 | `0x37` (3, 7) |
| 60 | 3, 9 | `0x69` | 185.74 | 61.91 | 1 | L (`0x0C`) | 23 | `0x35` (3, 5) |
| 61 | 3, 10 | `0x6A` | 204.79 | 61.91 | 1 | ; (`0x30`) | 36 | `0x5C` (5, 12) |
| 62 | 3, 11 | `0x6B` | 223.84 | 61.91 | 1 | ' (`0x31`) | 49 | `0x58` (5, 8) |
| 63 | 3, 12 | `0x6C` | 242.89 | 61.91 | 1 | \ (`0x2E`) | 62 | `0x57` (5, 7) |
| 65 | 3, 14 | `0x6E` | 264.32 | 52.39 | 1.25 x 2u high | Enter (`0x25`) | 75 | `0x56` (5, 6) |

Row 4 (y = 80.96 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 68 | 4, 0 | `0x80` | 11.91 | 80.96 | 2.25 | Left Shift (`0x64`) | 5 | `0x1B` (1, 11) |
| 70 | 4, 2 | `0x82` | 42.86 | 80.96 | 1 | Z (`0x1A`) | 31 | `0x10` (1, 0) |
| 71 | 4, 3 | `0x83` | 61.91 | 80.96 | 1 | X (`0x18`) | 44 | `0x12` (1, 2) |
| 72 | 4, 4 | `0x84` | 80.96 | 80.96 | 1 | C (`0x03`) | 57 | `0x13` (1, 3) |
| 73 | 4, 5 | `0x85` | 100.01 | 80.96 | 1 | V (`0x16`) | 70 | `0x14` (1, 4) |
| 74 | 4, 6 | `0x86` | 119.06 | 80.96 | 1 | B (`0x02`) | 83 | `0x3B` (3, 11) |
| 75 | 4, 7 | `0x87` | 138.11 | 80.96 | 1 | N (`0x0E`) | 96 | `0x3D` (3, 13) |
| 76 | 4, 8 | `0x88` | 157.16 | 80.96 | 1 | M (`0x0D`) | 11 | `0x30` (3, 0) |
| 77 | 4, 9 | `0x89` | 176.21 | 80.96 | 1 | , (`0x33`) | 24 | `0x31` (3, 1) |
| 78 | 4, 10 | `0x8A` | 195.26 | 80.96 | 1 | . (`0x34`) | 37 | `0x32` (3, 2) |
| 79 | 4, 11 | `0x8B` | 214.31 | 80.96 | 1 | / (`0x35`) | 50 | `0x5D` (5, 13) |
| 80 | 4, 12 | `0x8C` | 233.36 | 80.96 | 1 | International1 (`0xF2`) | 63 | `0x51` (5, 1) |
| 82 | 4, 14 | `0x8E` | 259.56 | 80.96 | 1.75 | Right Shift (`0x68`) | 89 | `0x5E` (5, 14) |
| 83 | 4, 15 | `0x8F` | 290.51 | 80.96 | 1 | Up (`0x4F`) | 76 | `0x55` (5, 5) |

Row 5 (y = 100.01 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 85 | 5, 0 | `0xA0` | 2.38 | 100.01 | 1.25 | Left Ctrl (`0x63`) | 6 | `0x1C` (1, 12) |
| 86 | 5, 1 | `0xA1` | 23.81 | 100.01 | 1 | Left GUI (`0x66`) | 18 | `0x1D` (1, 13) |
| 87 | 5, 2 | `0xA2` | 45.24 | 100.01 | 1.25 | Left Alt (`0x65`) | 19 | `0x1E` (1, 14) |
| 88 | 5, 3 | `0xA3` | 66.67 | 100.01 | 1 | International5 (`0xF6`) | 32 | `0x11` (1, 1) |
| 91 | 5, 6 | `0xA6` | 119.06 | 100.01 | 4.5 | Space (`0x29`) | 71 | `0x3C` (3, 12) |
| 94 | 5, 9 | `0xA9` | 173.83 | 100.01 | 1.25 | International4 (`0xF5`) | 97 | `0x3E` (3, 14) |
| 95 | 5, 10 | `0xAA` | 197.64 | 100.01 | 1.25 | International2 (`0xF3`) | 12 | `0x33` (3, 3) |
| 96 | 5, 11 | `0xAB` | 221.46 | 100.01 | 1.25 | Right Alt (`0x69`) | 25 | `0x34` (3, 4) |
| 97 | 5, 12 | `0xAC` | 242.89 | 100.01 | 1 | action 9 (momentary_layer1) (`0xE7`) | 38 | `0x50` (5, 0) |
| 99 | 5, 14 | `0xAE` | 271.46 | 100.01 | 1 | Left (`0x4D`) | 13 | `0x52` (5, 2) |
| 100 | 5, 15 | `0xAF` | 290.51 | 100.01 | 1 | Down (`0x4E`) | 26 | `0x53` (5, 3) |
| 101 | 5, 16 | `0xB0` | 309.56 | 100.01 | 1 | Right (`0x4C`) | 39 | `0x54` (5, 4) |

LED-only cells:

| Row, col | LED number | x mm | y mm |
| :---: | ---: | ---: | ---: |
| 5, 5 | 58 | 97.36 | 100.01 |
| 5, 7 | 84 | 140.76 | 100.01 |

Stabilizers:

| Cell | Key | Direction | Wire spacing mm | Part name in the design files |
| :---: | --- | --- | ---: | --- |
| 3, 14 | Enter | across the rows | 23.80 | `STABILIZER-2P_2_1` |
| 4, 0 | Left Shift | along the row | 23.80 | `STABILIZER-2P_2_1` |
| 5, 6 | Space | along the row | 66.67 | `STABILIZER-4P5_2_1` |

### Light bar

The feature `lightbar` drives a group of ten LEDs separately from the keys (`BOARD_LIGHTBAR_LEDS`, see [hardware](hardware.md#leds)):

- Ten LED bodies (2.80 x 5.92 x 1.85 mm) stand in one line at y = 61.86 mm, from x = 284.74 to 315.34 mm, 3.40 mm apart. This is the height of key row 3, to the right of the Enter key, above the Up arrow. The grid cells (3, 15) and (3, 16), which would be there, are empty in all three layout tables.
- The bodies sit under the circuit board and reach into a slot of 34.0 x 3.7 mm in the board (centre x = 300.04, y = 61.86 mm). They are 1.85 mm high, from 7.60 to 5.75 mm below the plate top; the board is between 6.60 and 5.00 mm below it.
- Above the slot, the plate has a window of 36.4 x 8.2 mm and the top case a window of 35.10 x 4.30 mm.
- A light guide with ten separate openings, one above each LED, leads to the visible bar (45.47 x 8.10 mm, from x = 277.64 to 323.12 mm and y = 57.81 to 65.91 mm, top face 7.60 mm above the plate).

Positions of the ten LEDs (the `lightbar` lines of `leds.csv`, in the order of the light-bar positions 0..9; position 0 is assumed to be the left end):

| Light-bar position | LED number | `x_mm` | `y_mm` |
| ---: | ---: | ---: | ---: |
| 0 | 51 | 284.737 | 61.862 |
| 1 | 64 | 288.137 | 61.862 |
| 2 | 77 | 291.537 | 61.862 |
| 3 | 90 | 294.937 | 61.862 |
| 4 | 103 | 298.337 | 61.862 |
| 5 | 52 | 301.737 | 61.862 |
| 6 | 65 | 305.137 | 61.862 |
| 7 | 78 | 308.537 | 61.862 |
| 8 | 91 | 311.937 | 61.862 |
| 9 | 104 | 315.337 | 61.862 |

**Which LED is which.** The firmware numbers the light-bar LEDs 0 to 9 and maps them to the LED numbers 51, 64, 77, 90, 103, 52, 65, 78, 91, 104. Which end of the bar is number 0 is not determined. The "mirror" light-bar effect copies a row of keys onto the bar with rising key columns going to rising bar numbers (`lightbar_paint_mirror` in `src/rgb/lightbar/lightbar_effects.c`), which fits "0 = left end, 9 = right end". To be certain, light one bar LED on a keyboard and look.

### USB connector and other details

- The USB-C socket is not on the main circuit board. It sits on a separate board of 22.0 x 18.0 x 1.2 mm at the rear left, from x = 1.06 to 23.06 mm and y = -14.84 to 3.16 mm, 2.1 mm below the underside of the main board. The socket opening points to the rear at x = 12.06 mm (between Esc and F1).
- The same small connector part (9.0 x 5.0 mm) appears twice: once on the USB board and once on the underside of the main board at x = 15.77 to 24.77 mm, y = 50.36 to 55.36 mm (near the left edge, between key rows 2 and 3).
- The main board is fixed to the plate by 15 screws; the board has 15 holes of 2.50 mm at the screw positions.
- One 2.70 mm hole (x = 114.78, y = 52.36 mm) and one slot of 3.70 x 2.70 mm (x = -5.24, y = 52.36 mm) have no screw.
- Four holes of 0.90 mm in a row, 2.54 mm apart, lie under the space bar: at x = 88.07 to 95.69 mm, y = 100.01 mm on ANSI and ISO, and at x = 116.44 to 124.06 mm, y = 104.01 mm on JIS.

### Not determined

- The design files do not label keys, and the firmware does not store positions. Each switch position was given its grid cell by order: within a row, the switch positions from left to right take the cells that the layout table of the variant populates in that row. The two-row Enter of ISO and JIS lies between rows 2 and 3 and takes cell (3, 14). The counts agree in every row of every variant, and every stabilized position receives a wide key of the factory key map.
- Key widths are not in the design files. They are derived from the openings of the top case, with each key centred on its switch.
- The order of the LED-only cells under the space bar. They are matched to the extra LED windows from left to right.
- The order of the ten light-bar LEDs along the bar (see [Light bar](#light-bar)).
- Where a hall sensor sits relative to its switch centre. The design files show the circuit board without components.
- Tolerances of a manufactured keyboard, and whether boards of other revisions differ from the published design files.
- What the connector between the USB board and the main board carries, and the purpose of the unnamed holes.

## Wooting 60HE+

**The positions in this section are nominal, and nothing of it is confirmed on hardware.** No mechanical data of the 60HE+ is available to this project. Row, column, key id, LED number, scan position and default key code are those of the board tables (taken from the board's stock firmware 2.14.1); the x and y values place each key on the usual 60 % grid (1u = 19.05 mm, rows 1u apart, keys of a row side by side without gaps), with the width that the usual 60 % layout gives the key. A real keyboard can differ in every position and width.

### Overview

| | ANSI | ISO |
| --- | ---: | ---: |
| Switch positions | 61 | 62 |
| Switch positions per row, rows 1 to 5 (row 0 has none) | 14, 14, 13, 12, 8 | 14, 13, 14, 13, 8 |
| Key LEDs (LED numbers in the key table) | 65 | 66 |
| Unused sensor channels (of 64) | 3 | 2 |
| Space bar width (nominal) | 6.25u | 6.25u |
| LED-only cells under the space bar | 4 | 4 |
| Light-bar LEDs | 0 | 0 |

The two variants use the same firmware; bit 0 of a user option byte of the microcontroller selects the tables. Nominal key rows: y = 0.00, 19.05, 38.10, 57.15, 76.20 mm for rows 1 to 5.

### Reference tables

#### ANSI

```text
row 1  [0 ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][  13  ]
row 2  [ 0  ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][ 13 ]
row 3  [  0  ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][   13  ]
row 4  [   0   ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][    13   ]
row 5  [ 0 ][ 1 ][ 2 ][           6           ][ 10][ 11][ 12][ 13]
```

Row 1 (nominal y = 0.00 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 14 | 1, 0 | `0x20` | 0.00 | 0.00 | 1 | Esc (`0x26`) | 0 | `0x0D` (0, 13) |
| 15 | 1, 1 | `0x21` | 19.05 | 0.00 | 1 | 1 (`0x1B`) | 1 | `0x04` (0, 4) |
| 16 | 1, 2 | `0x22` | 38.10 | 0.00 | 1 | 2 (`0x1C`) | 2 | `0x03` (0, 3) |
| 17 | 1, 3 | `0x23` | 57.15 | 0.00 | 1 | 3 (`0x1D`) | 3 | `0x02` (0, 2) |
| 18 | 1, 4 | `0x24` | 76.20 | 0.00 | 1 | 4 (`0x1E`) | 4 | `0x1B` (1, 11) |
| 19 | 1, 5 | `0x25` | 95.25 | 0.00 | 1 | 5 (`0x1F`) | 5 | `0x1A` (1, 10) |
| 20 | 1, 6 | `0x26` | 114.30 | 0.00 | 1 | 6 (`0x20`) | 6 | `0x17` (1, 7) |
| 21 | 1, 7 | `0x27` | 133.35 | 0.00 | 1 | 7 (`0x21`) | 7 | `0x16` (1, 6) |
| 22 | 1, 8 | `0x28` | 152.40 | 0.00 | 1 | 8 (`0x22`) | 8 | `0x2A` (2, 10) |
| 23 | 1, 9 | `0x29` | 171.45 | 0.00 | 1 | 9 (`0x23`) | 9 | `0x25` (2, 5) |
| 24 | 1, 10 | `0x2A` | 190.50 | 0.00 | 1 | 0 (`0x24`) | 90 | `0x24` (2, 4) |
| 25 | 1, 11 | `0x2B` | 209.55 | 0.00 | 1 | - (`0x2A`) | 91 | `0x38` (3, 8) |
| 26 | 1, 12 | `0x2C` | 228.60 | 0.00 | 1 | = (`0x2B`) | 92 | `0x36` (3, 6) |
| 27 | 1, 13 | `0x2D` | 257.18 | 0.00 | 2 | Backspace (`0x27`) | 107 | `0x35` (3, 5) |

Row 2 (nominal y = 19.05 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 28 | 2, 0 | `0x40` | 4.76 | 19.05 | 1.5 | Tab (`0x28`) | 10 | `0x0C` (0, 12) |
| 29 | 2, 1 | `0x41` | 28.57 | 19.05 | 1 | Q (`0x11`) | 11 | `0x06` (0, 6) |
| 30 | 2, 2 | `0x42` | 47.62 | 19.05 | 1 | W (`0x17`) | 12 | `0x05` (0, 5) |
| 31 | 2, 3 | `0x43` | 66.67 | 19.05 | 1 | E (`0x05`) | 13 | `0x1C` (1, 12) |
| 32 | 2, 4 | `0x44` | 85.72 | 19.05 | 1 | R (`0x12`) | 14 | `0x19` (1, 9) |
| 33 | 2, 5 | `0x45` | 104.78 | 19.05 | 1 | T (`0x14`) | 15 | `0x15` (1, 5) |
| 34 | 2, 6 | `0x46` | 123.83 | 19.05 | 1 | Y (`0x19`) | 16 | `0x14` (1, 4) |
| 35 | 2, 7 | `0x47` | 142.88 | 19.05 | 1 | U (`0x15`) | 17 | `0x2B` (2, 11) |
| 36 | 2, 8 | `0x48` | 161.93 | 19.05 | 1 | I (`0x09`) | 18 | `0x29` (2, 9) |
| 37 | 2, 9 | `0x49` | 180.97 | 19.05 | 1 | O (`0x0F`) | 19 | `0x26` (2, 6) |
| 38 | 2, 10 | `0x4A` | 200.03 | 19.05 | 1 | P (`0x10`) | 93 | `0x23` (2, 3) |
| 39 | 2, 11 | `0x4B` | 219.07 | 19.05 | 1 | [ (`0x2C`) | 94 | `0x39` (3, 9) |
| 40 | 2, 12 | `0x4C` | 238.12 | 19.05 | 1 | ] (`0x2D`) | 95 | `0x37` (3, 7) |
| 41 | 2, 13 | `0x4D` | 261.94 | 19.05 | 1.5 | \ (`0x2E`) | 98 | `0x34` (3, 4) |

Row 3 (nominal y = 38.10 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 42 | 3, 0 | `0x60` | 7.14 | 38.10 | 1.75 | Caps Lock (`0x36`) | 20 | `0x0B` (0, 11) |
| 43 | 3, 1 | `0x61` | 33.34 | 38.10 | 1 | A (`0x01`) | 21 | `0x08` (0, 8) |
| 44 | 3, 2 | `0x62` | 52.39 | 38.10 | 1 | S (`0x13`) | 22 | `0x07` (0, 7) |
| 45 | 3, 3 | `0x63` | 71.44 | 38.10 | 1 | D (`0x04`) | 23 | `0x1D` (1, 13) |
| 46 | 3, 4 | `0x64` | 90.49 | 38.10 | 1 | F (`0x06`) | 24 | `0x18` (1, 8) |
| 47 | 3, 5 | `0x65` | 109.54 | 38.10 | 1 | G (`0x07`) | 25 | `0x13` (1, 3) |
| 48 | 3, 6 | `0x66` | 128.59 | 38.10 | 1 | H (`0x08`) | 26 | `0x2D` (2, 13) |
| 49 | 3, 7 | `0x67` | 147.64 | 38.10 | 1 | J (`0x0A`) | 27 | `0x2C` (2, 12) |
| 50 | 3, 8 | `0x68` | 166.69 | 38.10 | 1 | K (`0x0B`) | 28 | `0x28` (2, 8) |
| 51 | 3, 9 | `0x69` | 185.74 | 38.10 | 1 | L (`0x0C`) | 29 | `0x27` (2, 7) |
| 52 | 3, 10 | `0x6A` | 204.79 | 38.10 | 1 | ; (`0x30`) | 96 | `0x3A` (3, 10) |
| 53 | 3, 11 | `0x6B` | 223.84 | 38.10 | 1 | ' (`0x31`) | 97 | `0x3D` (3, 13) |
| 55 | 3, 13 | `0x6D` | 254.79 | 38.10 | 2.25 | Enter (`0x25`) | 104 | `0x33` (3, 3) |

Row 4 (nominal y = 57.15 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 56 | 4, 0 | `0x80` | 11.91 | 57.15 | 2.25 | Left Shift (`0x64`) | 30 | `0x0A` (0, 10) |
| 58 | 4, 2 | `0x82` | 42.86 | 57.15 | 1 | Z (`0x1A`) | 32 | `0x00` (0, 0) |
| 59 | 4, 3 | `0x83` | 61.91 | 57.15 | 1 | X (`0x18`) | 33 | `0x1E` (1, 14) |
| 60 | 4, 4 | `0x84` | 80.96 | 57.15 | 1 | C (`0x03`) | 34 | `0x1F` (1, 15) |
| 61 | 4, 5 | `0x85` | 100.01 | 57.15 | 1 | V (`0x16`) | 35 | `0x10` (1, 0) |
| 62 | 4, 6 | `0x86` | 119.06 | 57.15 | 1 | B (`0x02`) | 36 | `0x12` (1, 2) |
| 63 | 4, 7 | `0x87` | 138.11 | 57.15 | 1 | N (`0x0E`) | 37 | `0x2E` (2, 14) |
| 64 | 4, 8 | `0x88` | 157.16 | 57.15 | 1 | M (`0x0D`) | 38 | `0x2F` (2, 15) |
| 65 | 4, 9 | `0x89` | 176.21 | 57.15 | 1 | , (`0x33`) | 39 | `0x20` (2, 0) |
| 66 | 4, 10 | `0x8A` | 195.26 | 57.15 | 1 | . (`0x34`) | 99 | `0x22` (2, 2) |
| 67 | 4, 11 | `0x8B` | 214.31 | 57.15 | 1 | / (`0x35`) | 100 | `0x3C` (3, 12) |
| 69 | 4, 13 | `0x8D` | 250.03 | 57.15 | 2.75 | Right Shift (`0x68`) | 101 | `0x32` (3, 2) |

Row 5 (nominal y = 76.20 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 70 | 5, 0 | `0xA0` | 2.38 | 76.20 | 1.25 | Left Ctrl (`0x63`) | 40 | `0x0E` (0, 14) |
| 71 | 5, 1 | `0xA1` | 26.19 | 76.20 | 1.25 | Left GUI (`0x66`) | 41 | `0x0F` (0, 15) |
| 72 | 5, 2 | `0xA2` | 50.01 | 76.20 | 1.25 | Left Alt (`0x65`) | 42 | `0x01` (0, 1) |
| 76 | 5, 6 | `0xA6` | 121.44 | 76.20 | 6.25 | Space (`0x29`) | 45 | `0x11` (1, 1) |
| 80 | 5, 10 | `0xAA` | 192.88 | 76.20 | 1.25 | Right Alt (`0x69`) | 48 | `0x21` (2, 1) |
| 81 | 5, 11 | `0xAB` | 216.69 | 76.20 | 1.25 | Application (`0x77`) | 49 | `0x3B` (3, 11) |
| 82 | 5, 12 | `0xAC` | 240.51 | 76.20 | 1.25 | Right Ctrl (`0x67`) | 102 | `0x31` (3, 1) |
| 83 | 5, 13 | `0xAD` | 264.32 | 76.20 | 1.25 | action 9 (momentary_layer1) (`0xE7`) | 103 | `0x30` (3, 0) |

LED-only cells (`leds.csv` gives no position for them):

| Row, col | LED number | x mm | y mm |
| :---: | ---: | ---: | ---: |
| 5, 4 | 43 | unknown | unknown |
| 5, 5 | 44 | unknown | unknown |
| 5, 7 | 46 | unknown | unknown |
| 5, 8 | 47 | unknown | unknown |

#### ISO

```text
row 1  [0 ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][  13  ]
row 2  [ 0  ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][13v ]
row 3  [  0  ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][12][ 13]
row 4  [ 0 ][1 ][2 ][3 ][4 ][5 ][6 ][7 ][8 ][9 ][10][11][    13   ]
row 5  [ 0 ][ 1 ][ 2 ][           6           ][ 10][ 11][ 12][ 13]
```

Row 1 (nominal y = 0.00 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 14 | 1, 0 | `0x20` | 0.00 | 0.00 | 1 | Esc (`0x26`) | 0 | `0x0D` (0, 13) |
| 15 | 1, 1 | `0x21` | 19.05 | 0.00 | 1 | 1 (`0x1B`) | 1 | `0x04` (0, 4) |
| 16 | 1, 2 | `0x22` | 38.10 | 0.00 | 1 | 2 (`0x1C`) | 2 | `0x03` (0, 3) |
| 17 | 1, 3 | `0x23` | 57.15 | 0.00 | 1 | 3 (`0x1D`) | 3 | `0x02` (0, 2) |
| 18 | 1, 4 | `0x24` | 76.20 | 0.00 | 1 | 4 (`0x1E`) | 4 | `0x1B` (1, 11) |
| 19 | 1, 5 | `0x25` | 95.25 | 0.00 | 1 | 5 (`0x1F`) | 5 | `0x1A` (1, 10) |
| 20 | 1, 6 | `0x26` | 114.30 | 0.00 | 1 | 6 (`0x20`) | 6 | `0x17` (1, 7) |
| 21 | 1, 7 | `0x27` | 133.35 | 0.00 | 1 | 7 (`0x21`) | 7 | `0x16` (1, 6) |
| 22 | 1, 8 | `0x28` | 152.40 | 0.00 | 1 | 8 (`0x22`) | 8 | `0x2A` (2, 10) |
| 23 | 1, 9 | `0x29` | 171.45 | 0.00 | 1 | 9 (`0x23`) | 9 | `0x25` (2, 5) |
| 24 | 1, 10 | `0x2A` | 190.50 | 0.00 | 1 | 0 (`0x24`) | 90 | `0x24` (2, 4) |
| 25 | 1, 11 | `0x2B` | 209.55 | 0.00 | 1 | - (`0x2A`) | 91 | `0x38` (3, 8) |
| 26 | 1, 12 | `0x2C` | 228.60 | 0.00 | 1 | = (`0x2B`) | 92 | `0x36` (3, 6) |
| 27 | 1, 13 | `0x2D` | 257.18 | 0.00 | 2 | Backspace (`0x27`) | 107 | `0x35` (3, 5) |

Row 2 (nominal y = 19.05 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 28 | 2, 0 | `0x40` | 4.76 | 19.05 | 1.5 | Tab (`0x28`) | 10 | `0x0C` (0, 12) |
| 29 | 2, 1 | `0x41` | 28.57 | 19.05 | 1 | Q (`0x11`) | 11 | `0x06` (0, 6) |
| 30 | 2, 2 | `0x42` | 47.62 | 19.05 | 1 | W (`0x17`) | 12 | `0x05` (0, 5) |
| 31 | 2, 3 | `0x43` | 66.67 | 19.05 | 1 | E (`0x05`) | 13 | `0x1C` (1, 12) |
| 32 | 2, 4 | `0x44` | 85.72 | 19.05 | 1 | R (`0x12`) | 14 | `0x19` (1, 9) |
| 33 | 2, 5 | `0x45` | 104.78 | 19.05 | 1 | T (`0x14`) | 15 | `0x15` (1, 5) |
| 34 | 2, 6 | `0x46` | 123.83 | 19.05 | 1 | Y (`0x19`) | 16 | `0x14` (1, 4) |
| 35 | 2, 7 | `0x47` | 142.88 | 19.05 | 1 | U (`0x15`) | 17 | `0x2B` (2, 11) |
| 36 | 2, 8 | `0x48` | 161.93 | 19.05 | 1 | I (`0x09`) | 18 | `0x29` (2, 9) |
| 37 | 2, 9 | `0x49` | 180.97 | 19.05 | 1 | O (`0x0F`) | 19 | `0x26` (2, 6) |
| 38 | 2, 10 | `0x4A` | 200.03 | 19.05 | 1 | P (`0x10`) | 93 | `0x23` (2, 3) |
| 39 | 2, 11 | `0x4B` | 219.07 | 19.05 | 1 | [ (`0x2C`) | 94 | `0x39` (3, 9) |
| 40 | 2, 12 | `0x4C` | 238.12 | 19.05 | 1 | ] (`0x2D`) | 95 | `0x37` (3, 7) |

Row 3 (nominal y = 38.10 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 42 | 3, 0 | `0x60` | 7.14 | 38.10 | 1.75 | Caps Lock (`0x36`) | 20 | `0x0B` (0, 11) |
| 43 | 3, 1 | `0x61` | 33.34 | 38.10 | 1 | A (`0x01`) | 21 | `0x08` (0, 8) |
| 44 | 3, 2 | `0x62` | 52.39 | 38.10 | 1 | S (`0x13`) | 22 | `0x07` (0, 7) |
| 45 | 3, 3 | `0x63` | 71.44 | 38.10 | 1 | D (`0x04`) | 23 | `0x1D` (1, 13) |
| 46 | 3, 4 | `0x64` | 90.49 | 38.10 | 1 | F (`0x06`) | 24 | `0x18` (1, 8) |
| 47 | 3, 5 | `0x65` | 109.54 | 38.10 | 1 | G (`0x07`) | 25 | `0x13` (1, 3) |
| 48 | 3, 6 | `0x66` | 128.59 | 38.10 | 1 | H (`0x08`) | 26 | `0x2D` (2, 13) |
| 49 | 3, 7 | `0x67` | 147.64 | 38.10 | 1 | J (`0x0A`) | 27 | `0x2C` (2, 12) |
| 50 | 3, 8 | `0x68` | 166.69 | 38.10 | 1 | K (`0x0B`) | 28 | `0x28` (2, 8) |
| 51 | 3, 9 | `0x69` | 185.74 | 38.10 | 1 | L (`0x0C`) | 29 | `0x27` (2, 7) |
| 52 | 3, 10 | `0x6A` | 204.79 | 38.10 | 1 | ; (`0x30`) | 96 | `0x3A` (3, 10) |
| 53 | 3, 11 | `0x6B` | 223.84 | 38.10 | 1 | ' (`0x31`) | 97 | `0x3D` (3, 13) |
| 54 | 3, 12 | `0x6C` | 242.89 | 38.10 | 1 | \ (`0x2E`) | 98 | `0x34` (3, 4) |
| 55 | 3, 13 | `0x6D` | 264.32 | 28.57 | 1.25 x 2u high | Enter (`0x25`) | 104 | `0x33` (3, 3) |

Row 4 (nominal y = 57.15 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 56 | 4, 0 | `0x80` | 2.38 | 57.15 | 1.25 | Left Shift (`0x64`) | 30 | `0x0A` (0, 10) |
| 57 | 4, 1 | `0x81` | 23.81 | 57.15 | 1 | Non-US \ (`0x61`) | 31 | `0x09` (0, 9) |
| 58 | 4, 2 | `0x82` | 42.86 | 57.15 | 1 | Z (`0x1A`) | 32 | `0x00` (0, 0) |
| 59 | 4, 3 | `0x83` | 61.91 | 57.15 | 1 | X (`0x18`) | 33 | `0x1E` (1, 14) |
| 60 | 4, 4 | `0x84` | 80.96 | 57.15 | 1 | C (`0x03`) | 34 | `0x1F` (1, 15) |
| 61 | 4, 5 | `0x85` | 100.01 | 57.15 | 1 | V (`0x16`) | 35 | `0x10` (1, 0) |
| 62 | 4, 6 | `0x86` | 119.06 | 57.15 | 1 | B (`0x02`) | 36 | `0x12` (1, 2) |
| 63 | 4, 7 | `0x87` | 138.11 | 57.15 | 1 | N (`0x0E`) | 37 | `0x2E` (2, 14) |
| 64 | 4, 8 | `0x88` | 157.16 | 57.15 | 1 | M (`0x0D`) | 38 | `0x2F` (2, 15) |
| 65 | 4, 9 | `0x89` | 176.21 | 57.15 | 1 | , (`0x33`) | 39 | `0x20` (2, 0) |
| 66 | 4, 10 | `0x8A` | 195.26 | 57.15 | 1 | . (`0x34`) | 99 | `0x22` (2, 2) |
| 67 | 4, 11 | `0x8B` | 214.31 | 57.15 | 1 | / (`0x35`) | 100 | `0x3C` (3, 12) |
| 69 | 4, 13 | `0x8D` | 250.03 | 57.15 | 2.75 | Right Shift (`0x68`) | 101 | `0x32` (3, 2) |

Row 5 (nominal y = 76.20 mm):

| Cell index | Row, col | Key id | x mm | y mm | Width u | Default key (code) | LED number | Scan position (input, channel) |
| ---: | :---: | :---: | ---: | ---: | ---: | --- | ---: | :---: |
| 70 | 5, 0 | `0xA0` | 2.38 | 76.20 | 1.25 | Left Ctrl (`0x63`) | 40 | `0x0E` (0, 14) |
| 71 | 5, 1 | `0xA1` | 26.19 | 76.20 | 1.25 | Left GUI (`0x66`) | 41 | `0x0F` (0, 15) |
| 72 | 5, 2 | `0xA2` | 50.01 | 76.20 | 1.25 | Left Alt (`0x65`) | 42 | `0x01` (0, 1) |
| 76 | 5, 6 | `0xA6` | 121.44 | 76.20 | 6.25 | Space (`0x29`) | 45 | `0x11` (1, 1) |
| 80 | 5, 10 | `0xAA` | 192.88 | 76.20 | 1.25 | Right Alt (`0x69`) | 48 | `0x21` (2, 1) |
| 81 | 5, 11 | `0xAB` | 216.69 | 76.20 | 1.25 | Application (`0x77`) | 49 | `0x3B` (3, 11) |
| 82 | 5, 12 | `0xAC` | 240.51 | 76.20 | 1.25 | Right Ctrl (`0x67`) | 102 | `0x31` (3, 1) |
| 83 | 5, 13 | `0xAD` | 264.32 | 76.20 | 1.25 | action 9 (momentary_layer1) (`0xE7`) | 103 | `0x30` (3, 0) |

LED-only cells:

| Row, col | LED number | x mm | y mm |
| :---: | ---: | ---: | ---: |
| 5, 4 | 43 | unknown | unknown |
| 5, 5 | 44 | unknown | unknown |
| 5, 7 | 46 | unknown | unknown |
| 5, 8 | 47 | unknown | unknown |

### Light bar

The 60HE+ has no light bar, and its board does not list the feature `lightbar`.

### Not determined

- Every position and every key width of this section: they are nominal grid values.
- Which keys have stabilizers, the size of the circuit board and of the plate, and where the LEDs sit relative to their switches.
- The positions of the four LED-only cells under the space bar, and their order from left to right.
- Whether the matrix cells and LED numbers of the tables match the physical keys of a keyboard: the tables are those of the stock firmware, and no 60HE+ has been examined.
