/*
 * keys_data.h - RAM globals of the keys module: actuation settings, rapid-trigger groups, advanced
 * keys and their run-time state, special-key action state, key event rate window.
 *
 * One extern per global with a description of its content, writers and readers. Definitions and
 * initial values: keys_data.c.
 */
#ifndef KEYS_DATA_H
#define KEYS_DATA_H

#include "keys/keys_types.h"

/* Key id (row << 5 | col) of a key found pressed in the current pass of keys_process_scan_pass,
 * 0xFF = none. startup_run_suspended_task uses it as remote-wakeup trigger. */
extern uint8_t g_keys_last_pressed_key_id;

/* Profile-wide rapid-trigger params: sensitivity (nanopb default 65), optional release sensitivity,
 * continuous flag. Written field by field by profile_apply_cfg; used by keys_update_press_state
 * for keys whose rt_params is NULL. */
extern RapidTriggerParams g_keys_rapid_trigger_params;

/* cfg byte 10 (profile_apply_cfg): index into the tap length table {1, 20, 40} ms; values > 2
 * mean 40 ms. */
extern uint8_t g_keys_tap_duration_preset;

/* Highest used index of g_keys_modtap_slots, -1 = none. Maintained by keys_process_scan_pass and
 * keys_modtap_release_slot. */
extern int8_t g_keys_modtap_slot_top;

/* Profile-wide actuation point in the distance encoding (cfg +0), used for keys whose record
 * actuation_point is 0 (keys_get_actuation_threshold). Written by profile_apply_cfg. */
extern uint16_t g_keys_actuation_point;

/* Profile-wide rapid-trigger switch (cfg +3), used by keys_update_press_state for keys without
 * per-key override. Written by profile_apply_cfg. */
extern uint8_t g_keys_rapid_trigger_enabled;

/* Number of non-zero entries of the last applied "act" table (keys_apply_actuation_map);
 * protocol_cmd_get_actuation_points answers in the dense format when it is > 50. */
extern uint8_t g_keys_per_key_actuation_count;

/* Last dispatched state per special-key action 0..14; compared with the pass context by
 * keys_apply_key_action (press edges) and keys_process_scan_pass (both edges). Cleared by
 * profile_activate. */
extern keys_fn_action_state_t g_keys_fn_action_states[KEYS_FN_ACTION_COUNT];

/* 1 = per-key rapid-trigger overrides come from the "prt2" group list g_keys_rt_groups; 0 = legacy
 * "prt" format. Set / cleared by profile_activate and the prt / prt2 protocol handlers. */
extern uint8_t g_keys_rt_groups_active;

/* RAM copy of profile file "prt2": up to 30 groups of up to 10 key ids with either own
 * rapid-trigger params or an on / off bool. Key records point into it
 * (analog_key_state_t.rt_params, keys_apply_rapid_trigger_groups). */
extern RapidTriggerGroups g_keys_rt_groups;

/* Run-time state of the advanced keys, same index as g_keys_advanced_keys entries; an element is
 * zeroed when its entry is (re)written (keys_apply_advanced_key_entry). */
extern keys_advanced_key_state_t g_keys_advanced_key_states[KEYS_ADVANCED_KEY_MAX];

/* RAM copy of profile file "ak2": up to 40 advanced-key entries of 16 bytes. */
extern AdvancedKeys g_keys_advanced_keys;

/* g_clock_uptime_ms of the previous keys_process_scan_pass; seeded by keys_task_init and
 * profile_activate. */
extern uint32_t g_keys_task_stamp_ms;

/* Pending mod-tap keys that another key press may resolve (the scan pass allocates a slot,
 * keys_modtap_release_slot frees it). */
extern keys_modtap_slot_t g_keys_modtap_slots[KEYS_MODTAP_SLOT_COUNT];

/* Set by keys_apply_legacy_rapid_trigger_keys from a legacy "prt" message: list count + number of
 * list entries with a non-zero mode. protocol_cmd_get_legacy_rapid_trigger_keys answers in map
 * format when it is > 50. */
extern uint8_t g_keys_rt_legacy_override_count;

#endif /* KEYS_DATA_H */
