/*
 * profile: general profile settings (file "<dir>/cfg", message ProfileConfig).
 *
 * The message has no RAM copy: profile_apply_cfg scatters it into the globals
 * of the key, HID and scan modules, profile_export_cfg collects it again.
 */
#include "profile/profile.h"

#include "adc_scan/adc_scan_data.h"
#include "analog/analog_data.h"
#include "core/run_state.h"
#include "hid/hid_data.h"
#include "keys/keys.h"
#include "rgb/rgb_data.h"
#include "rgb/rgb_frame.h"
#include "rgb/lightbar/lightbar.h"
#include "storage/storage.h"
#include "tables/analog_tables.h"

#define PROFILE_CFG_ENCODED_NAME "cfg"

/* Collects the live settings into a ProfileConfig (inverse of profile_apply_cfg). */
void profile_export_cfg(ProfileConfig *cfg)
{
    uint8_t scan_mode = (uint8_t)(g_run_state_flags >> RUN_STATE_SCAN_MODE_SHIFT);

    cfg->rapid_trigger_enabled = g_keys_rapid_trigger_enabled;
    cfg->scan_mode = (ScanMode)scan_mode;
    cfg->scan_mode_enabled = scan_mode != 0u;
    cfg->keyboard_enabled_in_gamepad_mode = g_hid_kbd_enabled_in_gamepad_mode;
    cfg->rapid_trigger_sensitivity = g_keys_rapid_trigger_params.sensitivity;
    cfg->actuation_point = g_keys_actuation_point;
    cfg->has_scan_mode = true;
    cfg->has_rapid_trigger_sensitivity = true;
    cfg->has_rapid_trigger_non_continuous = g_keys_rapid_trigger_params.has_continuous;
    cfg->rapid_trigger_non_continuous = g_keys_rapid_trigger_params.continuous ^ 1;
    if (g_keys_rapid_trigger_params.has_release_sensitivity) {
        cfg->has_rapid_trigger_release_sensitivity = true;
        cfg->rapid_trigger_release_sensitivity = g_keys_rapid_trigger_params.release_sensitivity;
    } else {
        cfg->has_rapid_trigger_release_sensitivity = false;
        cfg->rapid_trigger_release_sensitivity = 0u;
    }
    cfg->tap_duration = (TapDuration)g_keys_tap_duration_preset;
    cfg->has_tap_duration = true;
}

/*
 * Copies a ProfileConfig into the live settings. A changed scan mode is handed
 * to the ADC scan (applied at the start of its next cycle). refresh_rgb != 0:
 * repaint when the scan mode went from off to on or back.
 */
void profile_apply_cfg(const ProfileConfig *cfg, uint32_t refresh_rgb)
{
    g_keys_actuation_point = cfg->actuation_point;

    uint8_t old_scan_mode = (uint8_t)(g_run_state_flags >> RUN_STATE_SCAN_MODE_SHIFT);
    uint8_t scan_mode = 0u;
    if (cfg->scan_mode_enabled) {
        scan_mode = cfg->scan_mode != 0 ? 1u : 0u;
    }
    if (old_scan_mode != scan_mode) {
        g_run_state_flags = (uint8_t)((g_run_state_flags & ~RUN_STATE_SCAN_MODE_MASK)
                                        | ((scan_mode & 7u) << RUN_STATE_SCAN_MODE_SHIFT));
        if (g_adc_scan_mode_requested != scan_mode) {
            g_adc_scan_mode_requested = scan_mode;
            g_adc_scan_mode_change_pending = 1u;
        }
    }

    g_keys_rapid_trigger_params.has_release_sensitivity = cfg->has_rapid_trigger_release_sensitivity;
    g_keys_rapid_trigger_params.release_sensitivity = cfg->rapid_trigger_release_sensitivity;
    g_keys_rapid_trigger_enabled = cfg->rapid_trigger_enabled;
    g_keys_rapid_trigger_params.has_continuous = cfg->has_rapid_trigger_non_continuous;
    g_keys_rapid_trigger_params.continuous = cfg->rapid_trigger_non_continuous ^ 1;
    g_hid_kbd_enabled_in_gamepad_mode = cfg->keyboard_enabled_in_gamepad_mode;
    g_keys_rapid_trigger_params.sensitivity = cfg->rapid_trigger_sensitivity;
    g_keys_tap_duration_preset = (uint8_t)cfg->tap_duration;

    if (refresh_rgb != 0u) {
        uint8_t new_scan_mode = (uint8_t)(g_run_state_flags >> RUN_STATE_SCAN_MODE_SHIFT);
        if ((old_scan_mode != 0u) != (new_scan_mode != 0u)) {
            if (new_scan_mode != 0u) {
                rgb_paint_base_colors(0u);
            }
            lightbar_apply_scan_mode_change();
        }
    }
}

/*
 * Unsigned division as the Cortex-M4 UDIV instruction performs it with
 * divide-by-zero trapping off: a zero divisor gives 0.
 */
static inline uint32_t profile_udiv(uint32_t numerator, uint32_t divisor)
{
    return divisor != 0u ? numerator / divisor : 0u;
}

/*
 * Usable travel of a switch in 0.05 mm units: table value minus the optional
 * offset, 0 if the offset is not smaller.
 */
static uint8_t profile_switch_travel(const SwitchParams *params)
{
    uint8_t travel = k_analog_switch_travel[params->type.switch_type];
    uint8_t offset = params->has_travel_offset ? params->travel_offset : 0u;

    return travel > offset ? (uint8_t)(travel - offset) : 0u;
}

/*
 * Limits of the two rapid-trigger distances for one switch travel: the common
 * minimum and the defaults of the press and the release sensitivity.
 */
static void profile_rapid_trigger_limits(const ProfileConfig *cfg, uint8_t travel, uint32_t *min_q12,
                                         uint32_t *sensitivity_default, uint32_t *release_default)
{
    *min_q12 = profile_udiv(PROFILE_CFG_MIN_DISTANCE_NUM, travel);
    if (cfg->has_rapid_trigger_release_sensitivity) {
        *sensitivity_default = profile_udiv(PROFILE_CFG_RT_DEFAULT_NUM, travel);
        *release_default = profile_udiv(PROFILE_CFG_RT_RELEASE_DEFAULT_NUM, travel);
    } else {
        *sensitivity_default = profile_udiv(PROFILE_CFG_RT_DEFAULT_NO_RELEASE_NUM, travel);
        *release_default = *sensitivity_default;
    }
}

/*
 * Clamps the three distance fields against the installed switch types: the
 * default switch and every key group of the switch profiles.
 */
void profile_sanitize_cfg_distances(ProfileConfig *cfg)
{
    const SwitchProfiles *switches = &g_analog_switch_profiles;

    /* Actuation point: the largest minimum over all switch types. */
    int32_t actuation_min = (int32_t)profile_udiv(PROFILE_CFG_MIN_DISTANCE_NUM,
                                                  profile_switch_travel(&switches->default_params));
    for (uint8_t i = 0u; i < switches->entries_count; i++) {
        const SwitchProfileEntry *entry = &switches->entries[i];
        if (entry->which_kind != SwitchProfileEntry_key_group_tag) {
            continue;
        }
        int32_t entry_min = (int32_t)profile_udiv(PROFILE_CFG_MIN_DISTANCE_NUM,
                                                  profile_switch_travel(&entry->kind.key_group.params));
        if (actuation_min < entry_min) {
            actuation_min = entry_min;
        }
    }
    keys_sanitize_distance(&cfg->actuation_point, (uint32_t)actuation_min, PROFILE_CFG_ACTUATION_DEFAULT_Q12);

    /* Rapid-trigger distances.
     * KNOWN ISSUE: every key group overwrites the limits instead of widening them, so the
     * last key group (or the default switch if there is none) decides alone. Matches stock
     * firmware 2.14.1. */
    uint32_t rt_min;
    uint32_t sensitivity_default;
    uint32_t release_default;
    profile_rapid_trigger_limits(cfg, profile_switch_travel(&switches->default_params), &rt_min,
                                 &sensitivity_default, &release_default);
    for (uint8_t i = 0u; i < switches->entries_count; i++) {
        const SwitchProfileEntry *entry = &switches->entries[i];
        if (entry->which_kind != SwitchProfileEntry_key_group_tag) {
            continue;
        }
        profile_rapid_trigger_limits(cfg, profile_switch_travel(&entry->kind.key_group.params), &rt_min,
                                     &sensitivity_default, &release_default);
    }

    if (cfg->has_rapid_trigger_sensitivity) {
        keys_sanitize_distance(&cfg->rapid_trigger_sensitivity, rt_min, sensitivity_default);
    }
    if (cfg->has_rapid_trigger_release_sensitivity) {
        keys_sanitize_distance(&cfg->rapid_trigger_release_sensitivity, rt_min, release_default);
    }
}

/*
 * Reads the cfg file of a profile. Returns STORAGE_ERR_NOT_MOUNTED if the file
 * system is not mounted, else the result of storage_read_pb_file.
 */
static int profile_read_cfg(uint16_t profile_id, ProfileConfigFile *file)
{
    char path[16];

    if (g_storage_mounted == 0u) {
        return STORAGE_ERR_NOT_MOUNTED;
    }
    storage_format_profile_path(path, profile_id, PROFILE_CFG_ENCODED_NAME);
    return storage_read_pb_file(path, &ProfileConfigFile_msg, file);
}

/*
 * Loads and applies the cfg of a profile. Fallbacks: the cfg of built-in
 * profile 0, then the factory defaults of the profile index.
 */
void profile_load_and_apply_cfg(uint16_t profile_id)
{
    ProfileConfigFile file = { ProfileConfig_init_default };

    if (profile_read_cfg(profile_id, &file) != STORAGE_OK
        && profile_read_cfg(PROFILE_ID_RAW(0u, PROFILE_NS_BUILTIN), &file) != STORAGE_OK) {
        profile_fill_default_cfg(PROFILE_ID_INDEX(profile_id), &file.config);
    }
    profile_sanitize_cfg_distances(&file.config);
    profile_apply_cfg(&file.config, 0u);
}
