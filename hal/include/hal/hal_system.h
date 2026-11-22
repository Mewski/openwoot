/*
 * hal_system.h - the microcontroller as a whole: the set-up that the other areas of the
 * interface share, reset, and the messages exchanged with the bootloader across a reset.
 *
 * The bootloader and the firmware talk through a small store that survives a reset. The
 * firmware leaves an option for the next start (hal_system_restart), asks the bootloader
 * to stay in update mode (hal_system_enter_update_mode), and finds out whether the
 * bootloader asks for the settings volume to be formatted
 * (hal_system_take_format_request). Where that store is and how it is protected is the
 * business of the family.
 *
 * The clock tree is set up before main runs and is not part of this interface.
 *
 * Depends on: nothing.
 */
#ifndef HAL_SYSTEM_H
#define HAL_SYSTEM_H

#include <stdbool.h>
#include <stdint.h>

/* Switches on what the other areas of the interface share. Called once, as the first
 * step after hal_time_start_tick and before any other area is initialised. */
void hal_system_init(void);

/* Resets the microcontroller. Does not return. */
void hal_system_reset(void) __attribute__((noreturn));

/* Restarts the firmware: leaves `option` for the bootloader if it is not 0, then resets.
 * Does not return. */
void hal_system_restart(uint8_t option) __attribute__((noreturn));

/* Asks the bootloader to stay in update mode and leaves `option` for it, then resets.
 * Does not return. */
void hal_system_enter_update_mode(uint8_t option) __attribute__((noreturn));

/* True if the bootloader left a request to format the settings volume. The request is
 * cleared by this call, so that it is reported once. */
bool hal_system_take_format_request(void);

#endif /* HAL_SYSTEM_H */
