/*
 * wiring_template.h - the wiring constants of a board with a GD32F30x microcontroller.
 *
 * This file is not compiled. It is the list of constants that boards/<board>/wiring.h of
 * every board of this family defines, in this order; tools/check_tree.py compares the names.
 * To describe a new board, copy it to the board directory as wiring.h and give every constant
 * its value.
 *
 * Values use the names of the GD32F30x library: a port is GPIOx, a pin is GPIO_PIN_n; a list
 * is an initializer in braces, one entry per multiplexer or address line. Converter channels
 * and BOARD_MUX_ADDR_PORT_BASE are plain numbers. wiring.h is included by the files of
 * hal/gd32f30x/ only; inline/hal_scan_inline.h, which the firmware compiles as well, uses
 * BOARD_MUX_ADDR_PORT_BASE and nothing else of it. The pins of this family have no alternate
 * function numbers: a pin in an alternate mode belongs to the peripheral that is mapped to it.
 */
#ifndef WIRING_H
#define WIRING_H

/* Sensor scan: the port that carries all multiplexer address lines, the same port as its base
 * address (a number; hal_scan.c checks that the two agree), and the address line pins S0, S1,
 * ...; per multiplexer, its enable line (active low) and its converter input (pin and ADC0
 * channel); the zero-field reference input that the second converter measures together with
 * every sensor. */
#define BOARD_MUX_ADDR_PORT
#define BOARD_MUX_ADDR_PORT_BASE
#define BOARD_MUX_ADDR_PINS         /* BOARD_MUX_ADDR_LINES pins */
#define BOARD_MUX_ENABLE_PORTS      /* BOARD_MUX_COUNT ports */
#define BOARD_MUX_ENABLE_PINS       /* BOARD_MUX_COUNT pins */
#define BOARD_MUX_INPUT_PORTS       /* BOARD_MUX_COUNT ports */
#define BOARD_MUX_INPUT_PINS        /* BOARD_MUX_COUNT pins */
#define BOARD_MUX_INPUT_CHANNELS    /* BOARD_MUX_COUNT channel numbers */
#define BOARD_SCAN_REF_PORT
#define BOARD_SCAN_REF_PIN
#define BOARD_SCAN_REF_CHANNEL      /* converter channel of the reference input */

/* Settings flash on SPI1. */
#define BOARD_SPI_FLASH_MOSI_PORT
#define BOARD_SPI_FLASH_MOSI_PIN
#define BOARD_SPI_FLASH_SCK_PORT
#define BOARD_SPI_FLASH_SCK_PIN
#define BOARD_SPI_FLASH_MISO_PORT
#define BOARD_SPI_FLASH_MISO_PIN
#define BOARD_SPI_FLASH_CS_PORT     /* chip select, active low */
#define BOARD_SPI_FLASH_CS_PIN

/* LED driver chip on I2C0 (open drain) and its enable line. */
#define BOARD_LED_I2C_PORT          /* port of both bus lines */
#define BOARD_LED_I2C_SCL_PIN
#define BOARD_LED_I2C_SDA_PIN
#define BOARD_LED_ENABLE_PORT
#define BOARD_LED_ENABLE_PIN

/* USB full-speed controller: the switch of the D+ pull-up resistor (high = connected). */
#define BOARD_USB_PULLUP_PORT
#define BOARD_USB_PULLUP_PIN

#endif /* WIRING_H */
