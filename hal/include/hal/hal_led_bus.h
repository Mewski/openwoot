/*
 * hal_led_bus.h - the two-wire bus to the LED driver chip and the chip's enable line.
 *
 * The bus runs at 400 kHz with the microcontroller as the only master. A write is one
 * transfer: start condition, device address, `length` data bytes, stop condition. How the
 * family moves the bytes (polling, interrupt, DMA) is its own business; the call returns
 * when the stop condition is finished.
 *
 * Depends on: the board's wiring.h (pins), seen by the family implementation only. The
 * blocking waits run the USB device task (tud_task), so USB call-backs can run in the middle
 * of a write.
 */
#ifndef HAL_LED_BUS_H
#define HAL_LED_BUS_H

#include <stdbool.h>
#include <stdint.h>

/* The longest write, in bytes. A caller checks its lengths against it, at compile time where
 * they are bounded by a constant. */
#define HAL_LED_BUS_MAX_LENGTH 255u

/* Clocks, pins and the bus controller at 400 kHz; the enable line becomes an output. No
 * transfer is started and the enable line is not driven. */
void hal_led_bus_init(void);

/* One write transfer of `length` bytes (1 .. HAL_LED_BUS_MAX_LENGTH) from `data` to the
 * device with the 8-bit write address `address`. Blocking. Interrupts are masked from the
 * start condition until the data transfer runs. `data` must stay unchanged until the call
 * returns. */
void hal_led_bus_write(uint8_t address, const uint8_t *data, uint32_t length);

/* Drives the enable line of the LED driver chip: high for `on`. */
void hal_led_bus_set_enable(bool on);

#endif /* HAL_LED_BUS_H */
