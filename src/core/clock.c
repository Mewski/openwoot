/*
 * core: the millisecond counters and the busy wait on them.
 */
#include "core/clock.h"

/* Millisecond uptime; time base of every timestamp and periodic task. */
volatile uint32_t g_clock_uptime_ms;

/* Millisecond counter used only for busy waits: clock_delay_ms (the converter set-up of
 * the scan, one tick in startup_run_suspended_task) and the 100 ms wait of
 * protocol_cmd_factory_reset. */
volatile uint32_t g_clock_wait_ms;

void clock_delay_ms(uint32_t ms)
{
    uint32_t end = g_clock_wait_ms + ms;

    while (g_clock_wait_ms < end) {
    }
}
