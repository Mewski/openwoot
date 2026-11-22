/*
 * board.h - the Wooting 80HE in numbers: identity, image, key matrix, sensor scan, layout,
 * lighting and storage. This is the one place where these values are written; the modules in
 * src/ derive their own constants from them (LAYOUT_ROWS, ADC_SCAN_MUX_COUNT, RGB_LED_LIMIT, ...).
 *
 * Every board defines the same constants in the same order (tools/check_tree.py compares
 * them); behind them follows one block for each feature of board.mk that needs constants.
 * Dimensions are plain int constants: a module whose arithmetic needs an unsigned constant
 * converts the value in its own header. Pins are not here: they are in wiring.h and in
 * tables/adc_scan_tables.c, in the vocabulary of the microcontroller family.
 *
 * The board is described in docs/hardware.md, the key and LED positions in docs/layout.md.
 */
#ifndef BOARD_H
#define BOARD_H

/* ---- Identity ---------------------------------------------------------------------------------- */

#define BOARD_USB_VID               0x31E3u
#define BOARD_USB_PID_BASE          0x1400u /* idProduct = base | USB mode: 0x1400 XInput, 0x1401 HID gamepad,
                                             * 0x1402 without gamepad interface (the default) */
#define BOARD_USB_MANUFACTURER      "Wooting"
#define BOARD_USB_PRODUCT           "Wooting 80HE"

/* ---- Image ------------------------------------------------------------------------------------- */

/* linker.ld and board.json repeat these three values. */
#define BOARD_APP_BASE              0x08008000u /* start of the application image (vector table), behind the
                                                 * factory bootloader */
#define BOARD_FLASH_END             0x080FE000u /* end of the application area: the serial number page starts here */
#define BOARD_RAM_END               0x20030000u /* end of RAM: initial stack pointer */

/* ---- Key matrix -------------------------------------------------------------------------------- */

#define BOARD_KEY_ROWS              6    /* rows of the key matrix */
#define BOARD_KEY_COLS              17   /* columns of the key matrix that carry sensors */
#define BOARD_KEYMAP_COLS           21   /* columns per row in the stored key maps and base colour maps and in
                                          * the host messages; the columns from BOARD_KEY_COLS on are unused */
#define BOARD_KEY_POSITIONS         (BOARD_KEY_ROWS * BOARD_KEY_COLS) /* 102 matrix positions: size of the
                                                                       * key -> LED tables */
#define BOARD_KEY_FIRST_ROW         0    /* first row that carries keys */

/* ---- Scan -------------------------------------------------------------------------------------- */

#define BOARD_MUX_COUNT             6    /* 16:1 analog multiplexers; each output has its own converter
                                          * input */
#define BOARD_MUX_ADDR_LINES        4    /* address lines S0..S3, shared by all multiplexers */
#define BOARD_MUX_POSITIONS         16   /* positions per multiplexer (2 ^ BOARD_MUX_ADDR_LINES) */
#define BOARD_MUX_ADDRESSES_USED    15   /* addresses 0..14 carry sensors, address 15 is never selected */
#define BOARD_SCAN_POSITIONS        (BOARD_MUX_COUNT * BOARD_MUX_POSITIONS) /* 96; scan position = address +
                                                                             * BOARD_MUX_POSITIONS * multiplexer */

/* ---- Converter --------------------------------------------------------------------------------- */

#define BOARD_ADC_ZERO_LEVEL        2048 /* zero-field level of the sensors: the middle of the converter range */
#define BOARD_SCAN_OVERSAMPLE       1    /* conversions per sample: no averaging */
#define BOARD_SCAN_SAMPLE_HALF_CYCLES 6  /* sample time of a conversion in half converter clock cycles: 3 cycles */

/* ---- Layout ------------------------------------------------------------------------------------ */

#define BOARD_LAYOUT_VARIANT_COUNT  3    /* physical layouts: ANSI, ISO, JIS (k_layout_variants) */
#define BOARD_DEFAULT_KEYMAP_LAYERS 2    /* factory key map layers (k_layout_default_keymap_layers) */

/* ---- Lighting ---------------------------------------------------------------------------------- */

#define BOARD_LED_LIMIT             105  /* LED numbers are below this value: 1..104 carry the RGB LEDs of 8 switch lines
                                          * of 13 (k_rgb_led_channels), number 0 carries no LED */
#define BOARD_LED_NONE              0x69 /* 105: "no LED" entry of the key -> LED tables */
#define BOARD_RGB_FRAME_PAGE_COUNT  10   /* pages of the LED frame (k_rgb_frame_pages) */
#define BOARD_RGB_FRAME_PAGE_PWM_MAX 36  /* PWM bytes of the longest frame page */
#define BOARD_LED_GAIN_RED          255  /* stored value = (gain * (value + 1)) >> 8: all three channels use the full range */
#define BOARD_LED_GAIN_GREEN        255
#define BOARD_LED_GAIN_BLUE         255
#define BOARD_RGB_LEVEL_METER_SEGMENTS 10 /* keys of the level meter effect */

/* ---- LED driver -------------------------------------------------------------------------------- */

#define BOARD_LED_DRIVER_ADDRESS    0x60 /* bus address of the driver chip */
#define BOARD_LED_DRIVER_CONFIG_RUN 0x11 /* configuration register of the driver in operation: normal operation,
                                          * all switch lines scanned */

/* ---- Storage ----------------------------------------------------------------------------------- */

#define BOARD_STORAGE_LFS_BLOCK_COUNT 2047 /* 4 KiB blocks of the file system: the 8 MiB settings flash without its last
                                            * sector */
#define BOARD_DEVICE_INFO_ADDR      0x007FF000u /* factory device information: the last sector of the settings flash */
#define BOARD_MCU_SERIAL_PAGE_ADDR  0x080FE000u /* flash page of the microcontroller that mirrors the serial
                                                 * number */

/* ---- Feature: lightbar ------------------------------------------------------------------------- */

#define BOARD_LIGHTBAR_LED_COUNT    10   /* LEDs of the light bar; they are among the LEDs below BOARD_LED_LIMIT */
#define BOARD_LIGHTBAR_LEDS         51, 64, 77, 90, 103, 52, 65, 78, 91, 104 /* LED number of light-bar
                                                                              * position 0..9 */

/* ---- Feature: edition -------------------------------------------------------------------------- */

#define BOARD_EDITION_SERIAL_CODES  9, 99 /* edition codes of the serial number that mark a TenZ edition
                                          * unit */
#define BOARD_EDITION_USB_PRODUCT   "Wooting 80HE TenZ" /* product string of an edition unit */
#define BOARD_EDITION_FACTORY_PROFILE_COUNT 3 /* built-in profiles created at the first boot of an
                                               * edition unit */
#define BOARD_EDITION_DEFAULT_SWITCH_TYPE   1 /* switch type of keys without a switch profile entry
                                               * on an edition unit */

/* ---- Feature: recheck -------------------------------------------------------------------------- */

#define BOARD_SCAN_RECHECK_FIRST_INPUT 3 /* converter inputs from this one on are converted a second time
                                          * when their reading changed */

#endif /* BOARD_H */
