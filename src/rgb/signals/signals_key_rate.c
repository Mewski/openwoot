/*
 * signals_key_rate.c - signal RGB_SIGNAL_KEY_RATE: the number of key presses in a sliding
 * window of SIGNALS_KEY_RATE_SLOTS slots of 15 ms (990 ms). The key pass counts every press
 * into the current slot; the main loop moves the window on by one slot every 15 ms, drops what
 * the new slot counted one window ago and publishes the count of the window.
 *
 * Depends on core/clock.h (uptime) and core/startup.h (start-up step).
 */
#include <string.h>

#include "core/clock.h"
#include "core/startup.h"
#include "rgb/signals/signals.h"

#define SIGNALS_KEY_RATE_SLOTS     66u
#define SIGNALS_KEY_RATE_STEP_MS   15u

enum {
    STARTUP_SIGNALS_KEY_RATE = STARTUP_KEYS_TASK + 1, /* with the time state of the key task */
};

static uint32_t s_stamp_ms;                       /* uptime of the last window step */
static uint8_t s_slot;                            /* current slot */
static uint16_t s_sum;                            /* presses counted in all slots */
static uint8_t s_counts[SIGNALS_KEY_RATE_SLOTS];  /* presses per slot, saturating at 255 */

/* Start-up step: an empty window that starts now. */
static void signals_key_rate_reset(void)
{
    memset(s_counts, 0, sizeof(s_counts));
    s_stamp_ms = g_clock_uptime_ms;
    s_sum = 0;
    s_slot = 0;
}
STARTUP_STEP(STARTUP_SIGNALS_KEY_RATE, signals_key_rate_reset);

void signals_key_rate_note_press(void)
{
    uint8_t count = s_counts[s_slot];

    if (count != 0xFF) {
        s_counts[s_slot] = (uint8_t)(count + 1);
        s_sum++;
    }
}

void signals_key_rate_step(void)
{
    uint8_t slot;

    if (g_clock_uptime_ms - s_stamp_ms <= SIGNALS_KEY_RATE_STEP_MS - 1u) {
        return;
    }

    slot = (uint8_t)((s_slot + 1u) % SIGNALS_KEY_RATE_SLOTS);
    s_stamp_ms = g_clock_uptime_ms;
    s_slot = slot;
    s_sum = (uint16_t)(s_sum - s_counts[slot]);
    s_counts[slot] = 0;

    signals_publish_if_changed(RGB_SIGNAL_KEY_RATE, (float)s_sum);
}
