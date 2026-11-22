/*
 * analog_init.c - the start-up steps of the analog module: the per-key records
 * (g_analog_key_states) and the switch profiles.
 *
 * The scan interrupt writes the same records, but no conversion is triggered before the
 * start-up sweep, whose step comes after all of these.
 *
 * Depends on: core/startup.h, feature `edition` (default switch type of the unit).
 */
#include <string.h>

#include "analog/analog.h"
#include "core/startup.h"
#include "profile/edition/edition.h"

/*
 * Per key: sensor value 0, no per-key actuation point, "defer output" flag clear. */
static void analog_key_actuation_reset(void)
{
    for (uint32_t pos = 0; pos < ANALOG_KEY_COUNT; pos++) {
        analog_key_state_t *key = &g_analog_key_states[pos];

        key->value = 0;
        key->actuation_point = 0;
        key->flags &= (uint8_t)~ANALOG_KEY_FLAG_DEFER_OUTPUT;
    }
}
STARTUP_STEP(STARTUP_KEY_ACTUATION_RESET, analog_key_actuation_reset);

/*
 * Loads file "sw" into g_analog_switch_profiles; without it every key gets the default
 * switch type of the unit (no entries, default params {type}, file version 1). Then the
 * switch parameters are applied to the key records.
 */
static void analog_switch_profiles_init(void)
{
    if (analog_load_switch_profiles(&g_analog_switch_profiles) != 1) {
        g_analog_switch_profiles.entries_count = 0;
        g_analog_switch_profiles.has_default_params = true;
        g_analog_switch_profiles.default_params.which_type = SwitchParams_switch_type_tag;
        g_analog_switch_profiles.default_params.type.switch_type = profile_edition_default_switch_type();
        g_analog_switch_profiles.default_params.has_travel_offset = false;
        g_analog_switch_profiles.has_version = true;
        g_analog_switch_profiles.version = 1;
    }

    analog_apply_switch_profiles();
}
STARTUP_STEP(STARTUP_SWITCH_PROFILES, analog_switch_profiles_init);

/*
 * Start values of the calibration state of every key: no rest level yet (rest_level 0
 * and noise_band 0xFFFF are the running maximum / minimum of the start-up sweep), no
 * bottom-out knowledge, rebound filter idle, no recalibration running.
 */
static void analog_key_records_init(void)
{
    for (uint32_t pos = 0; pos < ANALOG_KEY_COUNT; pos++) {
        analog_key_state_t *key = &g_analog_key_states[pos];

        key->value = 0;
        key->rest_level = 0;
        key->noise_band = 0xFFFF;
        key->bottom_is_learned = 0;
        memset(&key->bo_tracker, 0, sizeof(key->bo_tracker));
        key->bo_reference = ANALOG_VALUE_NONE;
        key->bo_tracker.lower_bound = ANALOG_VALUE_NONE;
        key->bo_estimate = ANALOG_VALUE_NONE;
        key->rebound.prev_reading = ANALOG_VALUE_NONE;
        key->rebound.rest_timer_running = 0;
        key->rebound.rest_timer_start_us = 0;
        key->rebound.state = ANALOG_REBOUND_IDLE;
        key->recal_active = 0;
    }
}
STARTUP_STEP(STARTUP_KEY_RECORDS, analog_key_records_init);
