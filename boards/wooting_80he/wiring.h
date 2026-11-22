/*
 * wiring.h - the pins of the Wooting 80HE, in the vocabulary of its microcontroller family:
 * ports are GPIOx and pins GPIO_PIN_n of the GD32F4xx library; converter channels and
 * BOARD_MUX_ADDR_PORT_BASE are plain numbers.
 *
 * Included by the files of hal/gd32f4xx/ only; the constant list is that of
 * hal/gd32f4xx/wiring_template.h. inline/hal_scan_inline.h, which the firmware compiles as
 * well, uses the numeric BOARD_MUX_ADDR_PORT_BASE and nothing else of it. docs/hardware.md has
 * the complete pin map.
 */
#ifndef WIRING_H
#define WIRING_H

/* Sensor scan. Address lines S0..S3 of all multiplexers on PA8..PA11; their port also as its
 * base address. Per multiplexer 0..5: the enable line (active low) and the converter input
 * (pin and channel of ADC0). */
#define BOARD_MUX_ADDR_PORT         GPIOA
#define BOARD_MUX_ADDR_PORT_BASE    0x40020000u
#define BOARD_MUX_ADDR_PINS         { GPIO_PIN_8, GPIO_PIN_9, GPIO_PIN_10, GPIO_PIN_11 }
#define BOARD_MUX_ENABLE_PORTS      { GPIOB, GPIOB, GPIOC, GPIOC, GPIOC, GPIOC }
#define BOARD_MUX_ENABLE_PINS       { GPIO_PIN_14, GPIO_PIN_15, GPIO_PIN_6, GPIO_PIN_7, GPIO_PIN_8, GPIO_PIN_9 }
#define BOARD_MUX_INPUT_PORTS       { GPIOA, GPIOA, GPIOA, GPIOC, GPIOC, GPIOC }
#define BOARD_MUX_INPUT_PINS        { GPIO_PIN_0, GPIO_PIN_1, GPIO_PIN_2, GPIO_PIN_1, GPIO_PIN_5, GPIO_PIN_4 }
#define BOARD_MUX_INPUT_CHANNELS    { 0, 1, 2, 11, 15, 14 }

/* Settings flash on SPI0, alternate function 5. */
#define BOARD_SPI_FLASH_MOSI_PORT   GPIOA
#define BOARD_SPI_FLASH_MOSI_PIN    GPIO_PIN_7
#define BOARD_SPI_FLASH_SCK_PORT    GPIOB
#define BOARD_SPI_FLASH_SCK_PIN     GPIO_PIN_3
#define BOARD_SPI_FLASH_MISO_PORT   GPIOB
#define BOARD_SPI_FLASH_MISO_PIN    GPIO_PIN_4
#define BOARD_SPI_FLASH_CS_PORT     GPIOD       /* chip select, active low, idle high */
#define BOARD_SPI_FLASH_CS_PIN      GPIO_PIN_2

/* IS31FL3741 LED driver on I2C0, alternate function 4, open drain. */
#define BOARD_LED_I2C_PORT          GPIOB
#define BOARD_LED_I2C_SCL_PIN       GPIO_PIN_6
#define BOARD_LED_I2C_SDA_PIN       GPIO_PIN_7
#define BOARD_LED_ENABLE_PORT       GPIOB       /* presumably the enable input of the LED driver: set high once
                                                 * after the driver's software reset */
#define BOARD_LED_ENABLE_PIN        GPIO_PIN_8

/* USB high-speed transceiver (ULPI), alternate function 10. */
#define BOARD_USB_PHY_RESET_PORT    GPIOC       /* presumably the reset input of the transceiver: driven low at the
                                                 * start of hal_usb_prepare, high after the USB clocks are on */
#define BOARD_USB_PHY_RESET_PIN     GPIO_PIN_10
#define BOARD_ULPI_STP_PORT         GPIOC
#define BOARD_ULPI_STP_PIN          GPIO_PIN_0
#define BOARD_ULPI_DIR_PORT         GPIOC
#define BOARD_ULPI_DIR_PIN          GPIO_PIN_2
#define BOARD_ULPI_NXT_PORT         GPIOC
#define BOARD_ULPI_NXT_PIN          GPIO_PIN_3
#define BOARD_ULPI_CK_PORT          GPIOA
#define BOARD_ULPI_CK_PIN           GPIO_PIN_5
#define BOARD_ULPI_D0_PORT          GPIOA
#define BOARD_ULPI_D0_PIN           GPIO_PIN_3
#define BOARD_ULPI_D1_D7_PORT       GPIOB
#define BOARD_ULPI_D1_D7_PINS       (GPIO_PIN_0 | GPIO_PIN_1 | GPIO_PIN_5 | GPIO_PIN_10 | GPIO_PIN_11 | \
                                     GPIO_PIN_12 | GPIO_PIN_13)             /* D1, D2, D7, D3..D6 */

#endif /* WIRING_H */
