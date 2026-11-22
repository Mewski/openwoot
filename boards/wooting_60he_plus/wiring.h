/*
 * wiring.h - the pins of the Wooting 60HE+, in the vocabulary of its microcontroller family:
 * ports are GPIOx and pins GPIO_PIN_n of the GD32F30x library; converter channels and
 * BOARD_MUX_ADDR_PORT_BASE are plain numbers.
 *
 * Included by the files of hal/gd32f30x/ only; the constant list is that of
 * hal/gd32f30x/wiring_template.h. inline/hal_scan_inline.h, which the firmware compiles as
 * well, uses the numeric BOARD_MUX_ADDR_PORT_BASE and nothing else of it. README.md in this
 * directory has the complete pin map. No pin of this board has been confirmed on a device.
 */
#ifndef WIRING_H
#define WIRING_H

/* Sensor scan. Address lines S0..S3 of all multiplexers on PB0, PB1, PB10, PB11; their port
 * also as its base address. Per multiplexer 0..3: the enable line (active low) and the
 * converter input (pin and channel of ADC0). The zero-field reference input of the second
 * converter on PA4. */
#define BOARD_MUX_ADDR_PORT         GPIOB
#define BOARD_MUX_ADDR_PORT_BASE    0x40010C00u
#define BOARD_MUX_ADDR_PINS         { GPIO_PIN_0, GPIO_PIN_1, GPIO_PIN_10, GPIO_PIN_11 }
#define BOARD_MUX_ENABLE_PORTS      { GPIOB, GPIOB, GPIOC, GPIOC }
#define BOARD_MUX_ENABLE_PINS       { GPIO_PIN_8, GPIO_PIN_9, GPIO_PIN_13, GPIO_PIN_14 }
#define BOARD_MUX_INPUT_PORTS       { GPIOA, GPIOA, GPIOA, GPIOA }
#define BOARD_MUX_INPUT_PINS        { GPIO_PIN_0, GPIO_PIN_1, GPIO_PIN_2, GPIO_PIN_3 }
#define BOARD_MUX_INPUT_CHANNELS    { 0, 1, 2, 3 }
#define BOARD_SCAN_REF_PORT         GPIOA
#define BOARD_SCAN_REF_PIN          GPIO_PIN_4
#define BOARD_SCAN_REF_CHANNEL      4

/* Settings flash on SPI1. */
#define BOARD_SPI_FLASH_MOSI_PORT   GPIOB
#define BOARD_SPI_FLASH_MOSI_PIN    GPIO_PIN_15
#define BOARD_SPI_FLASH_SCK_PORT    GPIOB
#define BOARD_SPI_FLASH_SCK_PIN     GPIO_PIN_13
#define BOARD_SPI_FLASH_MISO_PORT   GPIOB
#define BOARD_SPI_FLASH_MISO_PIN    GPIO_PIN_14
#define BOARD_SPI_FLASH_CS_PORT     GPIOB       /* chip select, active low, idle high */
#define BOARD_SPI_FLASH_CS_PIN      GPIO_PIN_12

/* IS31FL3741 LED driver on I2C0, open drain. */
#define BOARD_LED_I2C_PORT          GPIOB
#define BOARD_LED_I2C_SCL_PIN       GPIO_PIN_6
#define BOARD_LED_I2C_SDA_PIN       GPIO_PIN_7
#define BOARD_LED_ENABLE_PORT       GPIOB       /* presumably the enable input of the LED driver: set high once
                                                 * after the driver's software reset */
#define BOARD_LED_ENABLE_PIN        GPIO_PIN_5

/* Switch of the USB D+ pull-up resistor: low = disconnected from the host, high = connected. */
#define BOARD_USB_PULLUP_PORT       GPIOA
#define BOARD_USB_PULLUP_PIN        GPIO_PIN_15

#endif /* WIRING_H */
