/*
 * is31fl3741.h - registers of the IS31FL3741 LED matrix driver chip and the two register
 * access functions of its driver (class interface: drivers/led_matrix.h).
 *
 * The chip has five register pages, selected through the command register after an unlock
 * write: two pages of PWM values (the banks of led_matrix.h), two of scaling values and the
 * function page.
 *
 * Depends on: hal_led_bus.h; the board's board.h for the bus address and the operating
 * configuration.
 */
#ifndef IS31FL3741_H
#define IS31FL3741_H

#include <stdint.h>

#define IS31FL3741_REG_COMMAND       0xFD /* page select */
#define IS31FL3741_REG_UNLOCK        0xFE /* command register write lock */
#define IS31FL3741_UNLOCK_KEY        0xC5
#define IS31FL3741_PAGE_PWM0         0    /* PWM registers 0..179 */
#define IS31FL3741_PAGE_PWM1         1    /* PWM registers 0..170 */
#define IS31FL3741_PAGE_SCALE0       2    /* scaling registers 0..179 */
#define IS31FL3741_PAGE_SCALE1       3    /* scaling registers 0..170 */
#define IS31FL3741_PAGE_FUNCTION     4
#define IS31FL3741_PAGE_UNKNOWN      0xFF /* start value of g_rgb_is31fl3741_page */
#define IS31FL3741_FN_CONFIG         0x00 /* configuration: BOARD_LED_DRIVER_CONFIG_RUN in operation, 0x00 = shutdown */
#define IS31FL3741_FN_GLOBAL_CURRENT 0x01
#define IS31FL3741_FN_PULL_UP_DOWN   0x02
#define IS31FL3741_FN_RESET          0x3F
#define IS31FL3741_CONFIG_SHUTDOWN   0x00
#define IS31FL3741_RESET_KEY         0xAE /* written to IS31FL3741_FN_RESET */
#define IS31FL3741_SCALE0_COUNT      180  /* scaling registers of page 2 */
#define IS31FL3741_SCALE1_COUNT      171  /* scaling registers of page 3 */
#define IS31FL3741_SCALE_FULL        0xFF
#define IS31FL3741_GLOBAL_CURRENT    0xFF
#define IS31FL3741_PULL_UP_DOWN      0x35

#define IS31FL3741_TX_BUF_SIZE       200  /* g_rgb_i2c_tx_buf: [0] register, [1..] data; longest write is 1 + 36 */

extern uint8_t g_rgb_is31fl3741_page;                   /* selected register page, IS31FL3741_PAGE_UNKNOWN at start */
extern uint8_t g_rgb_i2c_tx_buf[IS31FL3741_TX_BUF_SIZE]; /* [0] register, [1..] data of the current write */

/* Writes register `reg` of the selected page and the `len` registers that follow it in one
 * bus transfer. */
void is31fl3741_i2c_write(uint8_t reg, const uint8_t *data, uint32_t len);

/* Selects register page `page` (unlock, then command register); no bus traffic when the page
 * is already selected. */
void is31fl3741_select_page(uint8_t page);

#endif /* IS31FL3741_H */
