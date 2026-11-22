/*
 * hal_mcu.h - facts and services of the microcontroller chip itself: its unique id, its
 * read protection, its own program flash, its option bytes.
 *
 * Depends on: nothing.
 */
#ifndef HAL_MCU_H
#define HAL_MCU_H

#include <stdbool.h>
#include <stdint.h>

/* Reads the unique id of the chip as three 32-bit words, in the order in which the host
 * protocol reports them. A chip with a shorter id fills the rest with zero. */
void hal_mcu_read_unique_id(uint32_t id[3]);

/* False if the program flash can be read out through the debug port. Otherwise true, and
 * `*code` receives the raw protection code of the chip; `*code` is not written when the
 * result is false. */
bool hal_mcu_is_read_protected(uint8_t *code);

/* Reads `length` bytes of the program flash at `address` into `data`. For the flash regions that
 * the board description places outside the application image (serial number page, device
 * information page). */
void hal_mcu_flash_read(uint32_t address, uint8_t *data, uint32_t length);

/* Erases the flash page that starts at `address` and programs `length` bytes of `data`
 * into it. The flash is programmed in 16-bit units: an odd `length` is rounded up, and
 * `data` must be readable up to that even length. Returns false, without changing
 * anything, if this microcontroller does not write its own flash. */
bool hal_mcu_flash_write_page(uint32_t address, const uint8_t *data, uint32_t length);

/* The first option byte of the chip that is free for the user of the chip; 0xFF if the
 * chip has none. */
uint8_t hal_mcu_read_user_option(void);

#endif /* HAL_MCU_H */
