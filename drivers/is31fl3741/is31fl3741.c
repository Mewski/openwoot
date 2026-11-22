/*
 * is31fl3741.c - driver of the IS31FL3741 LED matrix driver chip (class: drivers/led_matrix.h).
 *
 * Every register write is one bus transfer: the register number followed by the data,
 * assembled in g_rgb_i2c_tx_buf. The selected page is cached so that a write to the page
 * already selected costs no extra transfer.
 */
#include "is31fl3741.h"

#include <string.h>

#include "board.h"
#include "drivers/led_matrix.h"
#include "hal/hal_led_bus.h"

uint8_t g_rgb_is31fl3741_page = IS31FL3741_PAGE_UNKNOWN;

uint8_t g_rgb_i2c_tx_buf[IS31FL3741_TX_BUF_SIZE];

_Static_assert(sizeof(g_rgb_i2c_tx_buf) == 200, "g_rgb_i2c_tx_buf"); /* matches stock firmware 2.14.1 */

/* Every write is the register number and its data from g_rgb_i2c_tx_buf, so no write is longer
 * than the buffer. */
_Static_assert(IS31FL3741_TX_BUF_SIZE <= HAL_LED_BUS_MAX_LENGTH, "a write must fit hal_led_bus_write");

void is31fl3741_i2c_write(uint8_t reg, const uint8_t *data, uint32_t len)
{
    g_rgb_i2c_tx_buf[0] = reg;
    memcpy(&g_rgb_i2c_tx_buf[1], data, len);
    hal_led_bus_write(BOARD_LED_DRIVER_ADDRESS, g_rgb_i2c_tx_buf, len + 1);
}

void is31fl3741_select_page(uint8_t page)
{
    uint8_t value;

    if (g_rgb_is31fl3741_page != page) {
        value = IS31FL3741_UNLOCK_KEY;
        is31fl3741_i2c_write(IS31FL3741_REG_UNLOCK, &value, 1);
        value = page;
        is31fl3741_i2c_write(IS31FL3741_REG_COMMAND, &value, 1);
        g_rgb_is31fl3741_page = page;
    }
}

/* One-byte register write on the selected page. */
static void is31fl3741_write_reg(uint8_t reg, uint8_t value)
{
    is31fl3741_i2c_write(reg, &value, 1);
}

/*
 * Software reset, configuration register = shutdown, enable line high, all 351 scaling
 * registers to 0xFF, global current 0xFF, pull-up/down register 0x35. Matches stock firmware
 * 2.14.1.
 */
void led_matrix_init(void)
{
    uint32_t reg;

    is31fl3741_select_page(IS31FL3741_PAGE_FUNCTION);
    is31fl3741_write_reg(IS31FL3741_FN_RESET, IS31FL3741_RESET_KEY);

    /* KNOWN ISSUE: the page cache still says "function page" after the software reset,
     * so this select sends nothing. If the reset also returns the chip's page register
     * to its default (the datasheet says the reset "resets all registers"), the 0x00
     * below lands in register 0 of that page.
     * Harmless either way: the configuration register is 0x00 (shutdown) after a reset
     * and the next select addresses the chip again. Matches stock firmware 2.14.1. */
    is31fl3741_select_page(IS31FL3741_PAGE_FUNCTION);
    is31fl3741_write_reg(IS31FL3741_FN_CONFIG, IS31FL3741_CONFIG_SHUTDOWN);

    hal_led_bus_set_enable(true);

    is31fl3741_select_page(IS31FL3741_PAGE_SCALE0);
    for (reg = 0; reg < IS31FL3741_SCALE0_COUNT; reg++) {
        is31fl3741_write_reg((uint8_t)reg, IS31FL3741_SCALE_FULL);
    }
    is31fl3741_select_page(IS31FL3741_PAGE_SCALE1);
    for (reg = 0; reg < IS31FL3741_SCALE1_COUNT; reg++) {
        is31fl3741_write_reg((uint8_t)reg, IS31FL3741_SCALE_FULL);
    }

    is31fl3741_select_page(IS31FL3741_PAGE_FUNCTION);
    is31fl3741_write_reg(IS31FL3741_FN_GLOBAL_CURRENT, IS31FL3741_GLOBAL_CURRENT);
    is31fl3741_select_page(IS31FL3741_PAGE_FUNCTION);
    is31fl3741_write_reg(IS31FL3741_FN_PULL_UP_DOWN, IS31FL3741_PULL_UP_DOWN);
}

/* Configuration register: BOARD_LED_DRIVER_CONFIG_RUN (normal operation and the switch lines
 * the board scans) or 0x00 (software shutdown). */
void led_matrix_set_running(bool on)
{
    is31fl3741_select_page(IS31FL3741_PAGE_FUNCTION);
    is31fl3741_write_reg(IS31FL3741_FN_CONFIG, on ? BOARD_LED_DRIVER_CONFIG_RUN : IS31FL3741_CONFIG_SHUTDOWN);
}

/* Bank 0 and 1 are the two PWM pages. */
void led_matrix_write(uint8_t bank, uint8_t first, const uint8_t *values, uint32_t count)
{
    is31fl3741_select_page((uint8_t)(IS31FL3741_PAGE_PWM0 + bank));
    is31fl3741_i2c_write(first, values, count);
}
