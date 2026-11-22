/*
 * board.h - the Wooting 60HE+ in numbers: identity, image, key matrix, sensor scan, layout,
 * lighting and storage. This is the one place where these values are written; the modules in
 * src/ derive their own constants from them (LAYOUT_ROWS, ADC_SCAN_MUX_COUNT, RGB_LED_LIMIT, ...).
 *
 * Every board defines the same constants in the same order (tools/check_tree.py compares
 * them); behind them follows one block for each feature of board.mk that needs constants.
 * Dimensions are plain int constants: a module whose arithmetic needs an unsigned constant
 * converts the value in its own header. Pins are not here: they are in wiring.h and in
 * tables/adc_scan_tables.c, in the vocabulary of the microcontroller family.
 *
 * This board has never run openwoot on hardware; see README.md in this directory.
 */
#ifndef BOARD_H
#define BOARD_H

/* ---- Identity ---------------------------------------------------------------------------------- */

#define BOARD_USB_VID               0x31E3u
#define BOARD_USB_PID_BASE          0x1320u /* idProduct = base | USB mode: 0x1320 XInput, 0x1321 HID gamepad,
                                             * 0x1322 without gamepad interface (the default) */
#define BOARD_USB_MANUFACTURER      "Wooting"
#define BOARD_USB_PRODUCT           "Wooting 60HE+"

/* ---- Image ------------------------------------------------------------------------------------- */

/* linker.ld and board.json repeat these three values. */
#define BOARD_APP_BASE              0x08007000u /* start of the application image (vector table), behind the
                                                 * factory bootloader */
#define BOARD_FLASH_END             0x080FD000u /* end of the application area: the serial number page starts here */
#define BOARD_RAM_END               0x20018000u /* end of RAM: initial stack pointer */

/* ---- Key matrix -------------------------------------------------------------------------------- */

#define BOARD_KEY_ROWS              6    /* rows of the key matrix */
#define BOARD_KEY_COLS              14   /* columns of the key matrix that carry sensors */
#define BOARD_KEYMAP_COLS           21   /* columns per row in the stored key maps and base colour maps and in
                                          * the host messages; the columns from BOARD_KEY_COLS on are unused */
#define BOARD_KEY_POSITIONS         (BOARD_KEY_ROWS * BOARD_KEY_COLS) /* 84 matrix positions: size of the
                                                                       * key -> LED tables */
#define BOARD_KEY_FIRST_ROW         1    /* first row that carries keys: row 0 is empty */

/* ---- Scan -------------------------------------------------------------------------------------- */

#define BOARD_MUX_COUNT             4    /* 16:1 analog multiplexers; each output has its own converter
                                          * input */
#define BOARD_MUX_ADDR_LINES        4    /* address lines S0..S3, shared by all multiplexers */
#define BOARD_MUX_POSITIONS         16   /* positions per multiplexer (2 ^ BOARD_MUX_ADDR_LINES) */
#define BOARD_MUX_ADDRESSES_USED    16   /* all addresses 0..15 carry sensors */
#define BOARD_SCAN_POSITIONS        (BOARD_MUX_COUNT * BOARD_MUX_POSITIONS) /* 64; scan position = address +
                                                                             * BOARD_MUX_POSITIONS * multiplexer */

/* ---- Converter --------------------------------------------------------------------------------- */

#define BOARD_ADC_ZERO_LEVEL        2482 /* zero-field level of the sensors, used while the measured reference reads 0 */
#define BOARD_SCAN_OVERSAMPLE       8    /* conversions per sample: the converter averages 8 */
#define BOARD_SCAN_SAMPLE_HALF_CYCLES 3  /* sample time of a conversion in half converter clock cycles: 1.5 cycles */

/* ---- Layout ------------------------------------------------------------------------------------ */

#define BOARD_LAYOUT_VARIANT_COUNT  2    /* physical layouts: ANSI, ISO (k_layout_variants) */
#define BOARD_DEFAULT_KEYMAP_LAYERS 3    /* factory key map layers (k_layout_default_keymap_layers) */

/* ---- Lighting ---------------------------------------------------------------------------------- */

#define BOARD_LED_LIMIT             108  /* LED numbers are below this value: 66 of the numbers 0..107 carry an RGB LED
                                          * (k_rgb_led_channels) */
#define BOARD_LED_NONE              0x6C /* 108: "no LED" entry of the key -> LED tables */
#define BOARD_RGB_FRAME_PAGE_COUNT  7    /* pages of the LED frame (k_rgb_frame_pages) */
#define BOARD_RGB_FRAME_PAGE_PWM_MAX 30  /* PWM bytes of the longest frame page */
#define BOARD_LED_GAIN_RED          232  /* stored value = (gain * (value + 1)) >> 8: red is limited to 0..232, green and
                                          * blue use the full range */
#define BOARD_LED_GAIN_GREEN        255
#define BOARD_LED_GAIN_BLUE         255
#define BOARD_RGB_LEVEL_METER_SEGMENTS 12 /* keys of the level meter effect */

/* ---- LED driver -------------------------------------------------------------------------------- */

#define BOARD_LED_DRIVER_ADDRESS    0x60 /* bus address of the driver chip */
#define BOARD_LED_DRIVER_CONFIG_RUN 0x31 /* configuration register of the driver in operation: normal operation,
                                          * switch lines 1..6 scanned */

/* ---- Storage ----------------------------------------------------------------------------------- */

#define BOARD_STORAGE_LFS_BLOCK_COUNT 2048 /* 4 KiB blocks of the file system: the whole 8 MiB settings flash */
#define BOARD_DEVICE_INFO_ADDR      0x080FF000u /* factory device information: a flash page of the microcontroller */
#define BOARD_MCU_SERIAL_PAGE_ADDR  0x080FD000u /* flash page of the microcontroller that mirrors the serial
                                                 * number */

#endif /* BOARD_H */
