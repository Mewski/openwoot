/*
 * analog_data.c - definitions of the RAM globals of the analog module: per-key records, switch
 * profiles, idle noise recalibration state.
 *
 * Objects in .data carry their initial value; objects in .bss have no initialiser.
 */
#include "analog/analog_data.h"

/* ---- .data ------------------------------------------------------------------------------------ */

uint8_t g_analog_recal_pattern = ANALOG_RECAL_PATTERN_MUX_PAIR;

/* ---- .bss ------------------------------------------------------------------------------------- */

SwitchProfiles g_analog_switch_profiles;
analog_key_state_t g_analog_key_states[ANALOG_KEY_COUNT];
volatile uint8_t g_analog_recal_batch;
analog_switch_params_t g_analog_default_switch_params;
uint8_t g_analog_recal_hold;
uint32_t g_analog_recal_hold_stamp_ms;
volatile uint8_t g_analog_recal_running;
uint8_t g_analog_recal_pass_done;
uint8_t g_analog_recal_done_single;
uint8_t g_analog_recal_done_parallel;
uint32_t g_analog_recal_pass_done_stamp_ms;
uint8_t g_analog_recal_pass_time_source;
uint32_t g_analog_recal_pass_start_ms;
uint8_t g_analog_recal_pass_timing;
uint32_t g_analog_recal_pass_end_time;

/* Object sizes. */
_Static_assert(sizeof(g_analog_recal_pattern) == 1, "g_analog_recal_pattern size");
_Static_assert(sizeof(g_analog_switch_profiles) == 672, "g_analog_switch_profiles size");
_Static_assert(sizeof(g_analog_key_states) == 0xAC * ANALOG_KEY_COUNT, "g_analog_key_states size");
_Static_assert(sizeof(g_analog_recal_batch) == 1, "g_analog_recal_batch size");
_Static_assert(sizeof(g_analog_default_switch_params) == 16, "g_analog_default_switch_params size");
_Static_assert(sizeof(g_analog_recal_hold) == 1, "g_analog_recal_hold size");
_Static_assert(sizeof(g_analog_recal_hold_stamp_ms) == 4, "g_analog_recal_hold_stamp_ms size");
_Static_assert(sizeof(g_analog_recal_running) == 1, "g_analog_recal_running size");
_Static_assert(sizeof(g_analog_recal_pass_done) == 1, "g_analog_recal_pass_done size");
_Static_assert(sizeof(g_analog_recal_done_single) == 1, "g_analog_recal_done_single size");
_Static_assert(sizeof(g_analog_recal_done_parallel) == 1, "g_analog_recal_done_parallel size");
_Static_assert(sizeof(g_analog_recal_pass_done_stamp_ms) == 4, "g_analog_recal_pass_done_stamp_ms size");
_Static_assert(sizeof(g_analog_recal_pass_time_source) == 1, "g_analog_recal_pass_time_source size");
_Static_assert(sizeof(g_analog_recal_pass_start_ms) == 4, "g_analog_recal_pass_start_ms size");
_Static_assert(sizeof(g_analog_recal_pass_timing) == 1, "g_analog_recal_pass_timing size");
_Static_assert(sizeof(g_analog_recal_pass_end_time) == 4, "g_analog_recal_pass_end_time size");
