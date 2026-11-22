/*
 * keys_data.c - definitions of the RAM globals of the keys module: actuation settings,
 * rapid-trigger groups, advanced keys and their run-time state, special-key action state, key
 * event rate window.
 *
 * Objects in .data carry their power-on value; objects in .bss have no initialiser and start
 * as zero. Each global is described at its declaration in keys_data.h.
 */
#include "keys/keys_data.h"

/* ---- .data ------------------------------------------------------------------------------------ */

uint8_t g_keys_last_pressed_key_id = 0xFF;
RapidTriggerParams g_keys_rapid_trigger_params = {65, false, 0, false, false};
uint8_t g_keys_tap_duration_preset = 2;
int8_t g_keys_modtap_slot_top = -1;

/* ---- .bss ------------------------------------------------------------------------------------- */

uint16_t g_keys_actuation_point;
uint8_t g_keys_rapid_trigger_enabled;
uint8_t g_keys_per_key_actuation_count;
keys_fn_action_state_t g_keys_fn_action_states[KEYS_FN_ACTION_COUNT];
uint8_t g_keys_rt_groups_active;
RapidTriggerGroups g_keys_rt_groups;
keys_advanced_key_state_t g_keys_advanced_key_states[KEYS_ADVANCED_KEY_MAX];
AdvancedKeys g_keys_advanced_keys;
uint32_t g_keys_task_stamp_ms;
keys_modtap_slot_t g_keys_modtap_slots[KEYS_MODTAP_SLOT_COUNT];
uint8_t g_keys_rt_legacy_override_count;

/* Object sizes the module relies on; a change of a type or of the settings schema shows up here. */
_Static_assert(sizeof(g_keys_last_pressed_key_id) == 1, "g_keys_last_pressed_key_id size");
_Static_assert(sizeof(g_keys_rapid_trigger_params) == 8, "g_keys_rapid_trigger_params size");
_Static_assert(sizeof(g_keys_tap_duration_preset) == 1, "g_keys_tap_duration_preset size");
_Static_assert(sizeof(g_keys_modtap_slot_top) == 1, "g_keys_modtap_slot_top size");
_Static_assert(sizeof(g_keys_actuation_point) == 2, "g_keys_actuation_point size");
_Static_assert(sizeof(g_keys_rapid_trigger_enabled) == 1, "g_keys_rapid_trigger_enabled size");
_Static_assert(sizeof(g_keys_per_key_actuation_count) == 1, "g_keys_per_key_actuation_count size");
_Static_assert(sizeof(g_keys_fn_action_states) == 30, "g_keys_fn_action_states size");
_Static_assert(sizeof(g_keys_rt_groups_active) == 1, "g_keys_rt_groups_active size");
_Static_assert(sizeof(g_keys_rt_groups) == 662, "g_keys_rt_groups size");
_Static_assert(sizeof(g_keys_advanced_key_states) == 480, "g_keys_advanced_key_states size");
_Static_assert(sizeof(g_keys_advanced_keys) == 642, "g_keys_advanced_keys size");
_Static_assert(sizeof(g_keys_task_stamp_ms) == 4, "g_keys_task_stamp_ms size");
_Static_assert(sizeof(g_keys_modtap_slots) == 60, "g_keys_modtap_slots size");
_Static_assert(sizeof(g_keys_rt_legacy_override_count) == 1, "g_keys_rt_legacy_override_count size");
