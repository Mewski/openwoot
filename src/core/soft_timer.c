/*
 * core: the software timers, their start-up step and their service.
 */
#include "core/soft_timer.h"

#include "core/clock.h"
#include "core/startup.h"

/* Expiry call-backs: [SOFT_TIMER_BRIGHTNESS_REPEAT] rgb_brightness_up_timer_cb /
 * rgb_brightness_down_timer_cb (set by the brightness key actions),
 * [SOFT_TIMER_RGB_SAVE] rgb_save_settings_timer_cb (set by rgb_start_and_activate_profile).
 * soft_timers_init clears both. */
soft_timer_cb_t g_soft_timer_callbacks[SOFT_TIMER_COUNT];

/* g_clock_uptime_ms of the last software-timer pass. */
uint32_t g_soft_timer_stamp_ms;

/* Bit n = software timer n is running. Starters set it with interrupts masked;
 * soft_timers_service clears it when a one-shot timer expires. */
uint8_t g_soft_timer_active_mask;

/* The software timers, index = enum soft_timer_id. */
soft_timer_t g_soft_timers[SOFT_TIMER_COUNT];

/* Bit n = software timer n has expired (set by soft_timers_service); cleared by the
 * starters. Nothing in this firmware reads it. */
uint8_t g_soft_timer_expired_mask;

/* Takes the current uptime as the time of the last pass and clears both call-backs. */
static void soft_timers_init(void)
{
    g_soft_timer_stamp_ms = g_clock_uptime_ms;
    g_soft_timer_callbacks[SOFT_TIMER_BRIGHTNESS_REPEAT] = NULL;
    g_soft_timer_callbacks[SOFT_TIMER_RGB_SAVE] = NULL;
}
STARTUP_STEP(STARTUP_SOFT_TIMERS, soft_timers_init);

/*
 * A timer runs while its bit in g_soft_timer_active_mask is set (the starters are
 * layout_key_action_brightness_up / _down and rgb_step_brightness). On expiry the
 * call-back runs; reload != 0 restarts the timer, otherwise it stops.
 */
void soft_timers_service(void)
{
    uint32_t n;

    if (g_clock_uptime_ms - g_soft_timer_stamp_ms <= SOFT_TIMER_PERIOD_MS - 1u) {
        return;
    }
    g_soft_timer_stamp_ms = g_clock_uptime_ms;

    if (g_soft_timer_active_mask == 0) {
        return;
    }

    for (n = 0; n < SOFT_TIMER_COUNT; n++) {
        soft_timer_t *timer = &g_soft_timers[n];
        uint8_t bit = (uint8_t)(1u << n);

        if ((g_soft_timer_active_mask & bit) == 0) {
            continue;
        }
        timer->remaining--;
        if (timer->remaining != 0) {
            continue;
        }

        g_soft_timer_expired_mask |= bit;
        if (g_soft_timer_callbacks[n] != NULL) {
            g_soft_timer_callbacks[n]();
        }
        if (timer->reload != 0) {
            timer->remaining = timer->reload;
        } else {
            g_soft_timer_active_mask &= (uint8_t)~bit;
        }
    }
}
