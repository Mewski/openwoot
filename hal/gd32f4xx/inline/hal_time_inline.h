/*
 * hal_time_inline.h - the microsecond count of the GD32F4xx family (interface: hal/hal_time.h).
 *
 * TIMER4 is a 32-bit timer: it counts the microseconds itself, from 0 to 0xFFFFFFFE, and
 * hal_time_micros is its counter register (hal_time_start_micros in hal_time.c starts it).
 * The count skips 0xFFFFFFFF at its wrap, about every 71.6 minutes. It is one register read,
 * so no part of it can lag. Matches stock firmware 2.14.1.
 *
 * Depends on: nothing. The one register is defined here; hal_time.c checks its address
 * against the peripheral library.
 */
#ifndef HAL_TIME_INLINE_H
#define HAL_TIME_INLINE_H

#include <stdint.h>

#define HAL_TIME_MICROS_COUNTER (*(volatile uint32_t *)0x40000C24u) /* TIMER4 counter (TIMER_CNT) */

__attribute__((always_inline))
static inline uint32_t hal_time_micros(void)
{
    return HAL_TIME_MICROS_COUNTER;
}

#endif /* HAL_TIME_INLINE_H */
