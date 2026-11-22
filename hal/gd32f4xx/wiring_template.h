/*
 * wiring_template.h - the wiring constants of a board with a GD32F4xx microcontroller.
 *
 * This file is not compiled. It is the list of constants that boards/<board>/wiring.h of
 * every board of this family defines, in this order; tools/check_tree.py compares the names.
 * To describe a new board, copy it to the board directory as wiring.h and give every constant
 * its value.
 *
 * Values use the names of the GD32F4xx library: a port is GPIOx, a pin or a set of pins is
 * GPIO_PIN_n (several joined with |); a list is an initializer in braces, one entry per
 * multiplexer or address line. Converter channels and BOARD_MUX_ADDR_PORT_BASE are plain
 * numbers. wiring.h is included by the files of hal/gd32f4xx/ only; inline/hal_scan_inline.h,
 * which the firmware compiles as well, uses BOARD_MUX_ADDR_PORT_BASE and nothing else of it.
 */
#ifndef WIRING_H
#define WIRING_H

/* Sensor scan: the port that carries all multiplexer address lines, the same port as its base
 * address (a number; hal_scan.c checks that the two agree), and the address line pins S0, S1,
 * ...; per multiplexer, its enable line (active low) and its converter input (pin and ADC0
 * channel). */
#define BOARD_MUX_ADDR_PORT
#define BOARD_MUX_ADDR_PORT_BASE
#define BOARD_MUX_ADDR_PINS         /* BOARD_MUX_ADDR_LINES pins */
#define BOARD_MUX_ENABLE_PORTS      /* BOARD_MUX_COUNT ports */
#define BOARD_MUX_ENABLE_PINS       /* BOARD_MUX_COUNT pins */
#define BOARD_MUX_INPUT_PORTS       /* BOARD_MUX_COUNT ports */
#define BOARD_MUX_INPUT_PINS        /* BOARD_MUX_COUNT pins */
#define BOARD_MUX_INPUT_CHANNELS    /* BOARD_MUX_COUNT channel numbers */

/* Settings flash on SPI0 (alternate function 5). */
#define BOARD_SPI_FLASH_MOSI_PORT
#define BOARD_SPI_FLASH_MOSI_PIN
#define BOARD_SPI_FLASH_SCK_PORT
#define BOARD_SPI_FLASH_SCK_PIN
#define BOARD_SPI_FLASH_MISO_PORT
#define BOARD_SPI_FLASH_MISO_PIN
#define BOARD_SPI_FLASH_CS_PORT     /* chip select, active low */
#define BOARD_SPI_FLASH_CS_PIN

/* LED driver chip on I2C0 (alternate function 4, open drain) and its enable line. */
#define BOARD_LED_I2C_PORT          /* port of both bus lines */
#define BOARD_LED_I2C_SCL_PIN
#define BOARD_LED_I2C_SDA_PIN
#define BOARD_LED_ENABLE_PORT
#define BOARD_LED_ENABLE_PIN

/* USB high-speed transceiver on the ULPI pins (alternate function 10). STP, DIR and NXT share
 * one port, CK and D0 another; hal_usb.c checks it. */
#define BOARD_USB_PHY_RESET_PORT    /* reset line of the transceiver */
#define BOARD_USB_PHY_RESET_PIN
#define BOARD_ULPI_STP_PORT
#define BOARD_ULPI_STP_PIN
#define BOARD_ULPI_DIR_PORT
#define BOARD_ULPI_DIR_PIN
#define BOARD_ULPI_NXT_PORT
#define BOARD_ULPI_NXT_PIN
#define BOARD_ULPI_CK_PORT
#define BOARD_ULPI_CK_PIN
#define BOARD_ULPI_D0_PORT
#define BOARD_ULPI_D0_PIN
#define BOARD_ULPI_D1_D7_PORT       /* the port of the data lines D1..D7 */
#define BOARD_ULPI_D1_D7_PINS

#endif /* WIRING_H */
