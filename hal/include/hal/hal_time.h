/*
 * hal_time.h - the two clocks of the firmware: a 1 ms tick and a microsecond count.
 *
 * The tick is an interrupt: once it is started the family calls hal_time_tick_isr every
 * millisecond, and the firmware keeps its millisecond counters there. The microsecond
 * count is a free-running counter that is read, never waited for by an interrupt; the
 * sensor scan measures its settling and cycle times with it.
 *
 * hal_time_micros is inline, because the scan interrupt reads it for every sample: each
 * family implements it in inline/hal_time_inline.h.
 *
 * Depends on: the family's inline header; the firmware provides hal_time_tick_isr.
 */
#ifndef HAL_TIME_H
#define HAL_TIME_H

#include <stdint.h>

/* Starts the tick: from now on hal_time_tick_isr runs every millisecond, at the lowest
 * interrupt priority. */
void hal_time_start_tick(void);

/* Call-back, provided by the firmware: one millisecond has passed. Runs in interrupt
 * context. */
void hal_time_tick_isr(void);

/* Starts the microsecond count. hal_time_micros is meaningful from here on. */
void hal_time_start_micros(void);

/* The free-running microsecond count. It wraps around; a duration is the difference of
 * two readings, and the interrupt of the sensor scan takes its durations from the low 16
 * bits. The count skips one value where the family's counter wraps, so a duration measured
 * across that point is 1 us too long; this matches stock firmware 2.14.1. The family's
 * inline header says where that point is and in which contexts the bits above the counter
 * can lag. */
static inline uint32_t hal_time_micros(void);

#include "hal_time_inline.h"

#endif /* HAL_TIME_H */
