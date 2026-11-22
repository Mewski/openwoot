/*
 * hal_time_inline.h - the microsecond count of the GD32F30x family (interface: hal/hal_time.h).
 *
 * TIMER4 is a 16-bit timer: it counts the low half, from 0 to 0xFFFE (so the count skips
 * 0xFFFF in its low half at every wrap, every 65.535 ms), and its overflow interrupt
 * (TIMER4_IRQHandler in hal_time.c, priority 0) counts the high half in g_hal_time_micros_high.
 *
 * hal_time_micros reads the high half and then the counter, in the order of stock firmware
 * 2.14.1, and then the high half once more, which the stock code does not: if it changed, the
 * overflow interrupt ran between the reads and both are read again. Guarantee: wherever the
 * overflow interrupt can run (thread context with interrupts enabled, an interrupt of lower
 * priority), the result is exact. Where it cannot (interrupts masked, or an interrupt of
 * priority 0 such as that of the sensor scan), an overflow that is pending is not counted yet,
 * so the high half can be one behind, as in the stock code; the low 16 bits, the only part the
 * scan path uses, are exact in every context.
 *
 * Depends on: g_hal_time_micros_high (hal_time.c). The one register is defined here;
 * hal_time.c checks its address against the peripheral library.
 */
#ifndef HAL_TIME_INLINE_H
#define HAL_TIME_INLINE_H

#include <stdint.h>

#define HAL_TIME_MICROS_COUNTER (*(volatile uint32_t *)0x40000C24u) /* TIMER4 counter (TIMER_CNT) */

/* Overflows of TIMER4 since hal_time_start_micros. */
extern volatile uint16_t g_hal_time_micros_high;

__attribute__((always_inline))
static inline uint32_t hal_time_micros(void)
{
    uint32_t high;
    uint32_t low;

    do {
        high = g_hal_time_micros_high;
        low = HAL_TIME_MICROS_COUNTER;
    } while (high != g_hal_time_micros_high);
    return (high << 16) | low;
}

#endif /* HAL_TIME_INLINE_H */
