/*
 * core: the millisecond clock of the firmware. Two counters advance once per millisecond:
 * the uptime, which is the time base of every timestamp and period, and a second counter
 * that only the busy waits use. Both are written by the tick call-back alone
 * (hal_time_tick_isr, src/app/tasks.c) and read everywhere with single 32-bit loads.
 *
 * Depends on: nothing.
 */
#ifndef CORE_CLOCK_H_
#define CORE_CLOCK_H_

#include <stdint.h>

extern volatile uint32_t g_clock_uptime_ms;   /* time base of timestamps and periods */
extern volatile uint32_t g_clock_wait_ms;     /* millisecond counter of the busy waits */

/* Busy wait for `ms` milliseconds of the tick; with ms = 1 it ends at the next tick. Never
 * returns while the tick interrupt is masked. */
void clock_delay_ms(uint32_t ms);

#endif /* CORE_CLOCK_H_ */
