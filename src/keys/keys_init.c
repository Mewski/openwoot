/*
 * keys_init.c - the start-up step of the keys module: the time state of the key task.
 *
 * Depends on: core/clock.h (uptime), core/startup.h.
 */
#include "core/clock.h"
#include "core/startup.h"
#include "keys/keys_data.h"

/* Time stamp of the key pass (keys_process_scan_pass measures the elapsed time from it). */
static void keys_task_init(void)
{
    g_keys_task_stamp_ms = g_clock_uptime_ms;
}
STARTUP_STEP(STARTUP_KEYS_TASK, keys_task_init);
