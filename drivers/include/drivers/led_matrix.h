/*
 * led_matrix.h - what the lighting needs from an LED matrix driver chip.
 *
 * The chip holds one brightness value per LED channel in numbered banks of registers. The
 * lighting sends a run of values into one bank and switches the chip between running and
 * shut down (LEDs dark, registers kept). Exactly one driver of the board's DRIVERS list
 * implements this header.
 *
 * Depends on: nothing; the driver behind it uses hal_led_bus.h and the board's board.h.
 */
#ifndef LED_MATRIX_H
#define LED_MATRIX_H

#include <stdbool.h>
#include <stdint.h>

/* Brings the chip up after hal_led_bus_init: reset, shut down, enable line on, the chip's
 * fixed settings. The chip stays shut down until led_matrix_set_running(true). */
void led_matrix_init(void);

/* Running (with the board's operating configuration) or shut down. */
void led_matrix_set_running(bool on);

/* Writes `count` brightness values to bank `bank`, starting at value number `first`. */
void led_matrix_write(uint8_t bank, uint8_t first, const uint8_t *values, uint32_t count);

#endif /* LED_MATRIX_H */
