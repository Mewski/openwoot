/*
 * rgb_init.c - the start-up steps of the lighting: the bus and the LED driver chip, which
 * stays in shutdown until the main loop wakes it, and the first profile activation with the
 * time base of the lighting. The page layout of the LED frame is a step of rgb_frame.c.
 *
 * Depends on: core/clock.h (uptime), core/soft_timer.h (the save timer), core/startup.h,
 * hal_led_bus, the LED matrix driver class, the profile module.
 */
#include "core/clock.h"
#include "core/soft_timer.h"
#include "core/startup.h"
#include "drivers/led_matrix.h"
#include "hal/hal_led_bus.h"
#include "profile/profile.h"
#include "rgb/rgb_frame.h"

STARTUP_STEP(STARTUP_LED_BUS, hal_led_bus_init);
STARTUP_STEP(STARTUP_LED_MATRIX, led_matrix_init);

/*
 * Starts the rgb time base, installs the "save rgb settings" callback of software
 * timer 1 (armed by rgb_step_brightness), empties the overlay list and activates the
 * profile that is current in .data (profile 0 of the built-in namespace).
 */
static void rgb_start_and_activate_profile(void)
{
    profile_id_t active_id = g_profile_active_id;

    g_rgb_frame_stamp_ms = g_clock_uptime_ms;
    g_rgb_effect_stamp_ms = g_clock_uptime_ms;
    g_soft_timer_callbacks[SOFT_TIMER_RGB_SAVE] = rgb_save_settings_timer_cb;
    g_rgb_overlay_count = 0;

    profile_activate(active_id.raw);
}
STARTUP_STEP(STARTUP_PROFILE_ACTIVATE, rgb_start_and_activate_profile);
