/*
 * core: the software timers. A small fixed set of timers counts in periods of
 * SOFT_TIMER_PERIOD_MS; the main loop services them (soft_timers_service), and on expiry
 * a timer's call-back runs in main-loop context. A module starts a timer by setting its
 * count and its bit in g_soft_timer_active_mask with interrupts masked, and sets the
 * call-back in its slot. Each timer belongs to one user, named at its id.
 *
 * Depends on: core/clock.h (the uptime) and core/startup.h (the start-up step that resets
 * the timers).
 */
#ifndef CORE_SOFT_TIMER_H_
#define CORE_SOFT_TIMER_H_

#include <stddef.h>
#include <stdint.h>

#define SOFT_TIMER_PERIOD_MS    25u   /* a pass runs when uptime - stamp > 24 */

enum soft_timer_id {
    SOFT_TIMER_BRIGHTNESS_REPEAT = 0, /* brightness key auto repeat: remaining = reload = 2 (50 ms) */
    SOFT_TIMER_RGB_SAVE          = 1, /* one-shot 80 units (2 s) -> rgb_save_settings_timer_cb, started by rgb_step_brightness */
    SOFT_TIMER_COUNT             = 2,
};

/* One software timer, units of SOFT_TIMER_PERIOD_MS. */
typedef struct {
    uint16_t remaining; /* 0x0: decremented per pass while the active bit is set; expiry at 0 */
    uint16_t reload;    /* 0x2: copied to remaining on expiry; 0 = one-shot, the active bit is cleared */
} soft_timer_t;

_Static_assert(sizeof(soft_timer_t) == 4, "soft_timer_t size");
_Static_assert(offsetof(soft_timer_t, remaining) == 0, "soft_timer_t.remaining");
_Static_assert(offsetof(soft_timer_t, reload) == 2, "soft_timer_t.reload");

/* Expiry call-back, called from the main loop; NULL = none. */
typedef void (*soft_timer_cb_t)(void);

extern soft_timer_cb_t g_soft_timer_callbacks[SOFT_TIMER_COUNT];
extern uint32_t g_soft_timer_stamp_ms;      /* uptime of the last timer pass */
extern uint8_t g_soft_timer_active_mask;    /* bit n = timer n is running */
extern soft_timer_t g_soft_timers[SOFT_TIMER_COUNT];
extern uint8_t g_soft_timer_expired_mask;   /* bit n = timer n has expired */

/* One pass of the timers every SOFT_TIMER_PERIOD_MS; called on every pass of the main loop. */
void soft_timers_service(void);

#endif /* CORE_SOFT_TIMER_H_ */
