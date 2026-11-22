/*
 * profile: RAM globals. Objects in the first group have an initial value,
 * those in the second group start as zero.
 */
#include "profile_data.h"

/* ---- initialised ---------------------------------------------------------- */

/* Built-in profile the "toggle profile" key (key action 8) switches to from
 * built-in profile 0: the last non-zero built-in profile that was active.
 * Written by profile_activate (ns always 0) and by the key handler, clamped by
 * protocol_delete_last_profile. */
profile_id_t g_profile_toggle_target_id = { .index = 1, .ns = PROFILE_NS_BUILTIN };

/* Built-in profile that was active before a namespace-1 ("lp/<n>") profile was
 * loaded; target when leaving namespace 1 (set by profile_activate, used by the
 * profile keys and protocol_cmd_select_lp_profile). */
profile_id_t g_profile_lp_return_id = { .index = 0, .ns = PROFILE_NS_BUILTIN };

/* 1 while the host has selected a namespace-1 profile with feature command 84
 * (protocol_cmd_select_lp_profile); the id is g_profile_lp_selected_id. */
uint8_t g_profile_lp_selected = 0;

/* Namespace-1 profile selected by feature command 84; entered by the profile keys. */
profile_id_t g_profile_lp_selected_id = { .index = 0, .ns = PROFILE_NS_BUILTIN };

/* 1 = startup_run_active_task must call profile_activate(g_profile_switch_target.raw)
 * and clear the flag. */
uint8_t g_profile_switch_pending = 0;

/* Profile to activate when g_profile_switch_pending is set. */
profile_id_t g_profile_switch_target = { .index = 0, .ns = PROFILE_NS_BUILTIN };

/* Active profile; written only by profile_activate. */
profile_id_t g_profile_active_id = { .index = 0, .ns = PROFILE_NS_BUILTIN };

/* Number of existing profile directories per namespace: [0] "<n>" (max 4),
 * [1] "lp/<n>" (max 10). Refreshed by profile_counts_init with storage_count_profile_dirs
 * and by the create / delete handlers. */
uint8_t g_profile_counts[PROFILE_NS_COUNT] = { 4, 10 };

/* RAM copy of the root file "global" (field map: profile_types.h). The initial
 * value equals the nanopb defaults of the message, which global_settings_load and
 * protocol_cmd_set_global_settings also write before decoding. */
GlobalSettings g_profile_global_settings = {
    false,                              /* 0x00 has idle sub-message */
    {
        PROFILE_GLOBAL_IDLE_DIM_MINUTES_DEFAULT, /* 0x01 minutes until dim */
        PROFILE_GLOBAL_IDLE_OFF_MINUTES_DEFAULT, /* 0x02 minutes until off */
        false,                          /* 0x03 has switch-off enable */
        true,                           /* 0x04 switch-off enabled */
    },
    false,                              /* 0x05 has Num Lock indicator polarity */
    false,                              /* 0x06 Num Lock indicator polarity */
    false,                              /* 0x07 has option 3 */
    false,                              /* 0x08 option 3 */
    false,                              /* 0x09 has string */
    "",                                 /* 0x0A string[41] */
};

/* ---- zero-initialised ----------------------------------------------------- */

/* Latched (has option 3 && option 3) of g_profile_global_settings, set by
 * rebound_filter_option_init and protocol_cmd_set_global_settings. Read by the scan
 * paths, which run in the ADC interrupt. */
uint8_t g_profile_global_option3_active;

/* Cached device-info tag 4, see PROFILE_LOCK_MODE_IS_LOCKED. Written by device_info_load_lock_mode
 * and protocol_cmd_get_lock_mode / _set_lock_mode. */
uint8_t g_profile_lock_mode;
