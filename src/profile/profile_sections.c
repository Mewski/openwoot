/*
 * profile: the sections of a profile. Each section loads one part of the profile
 * being activated (mostly one file) into the RAM state of the module that owns it and
 * is registered in the profile_sections list next to its function. A missing or
 * unreadable file means "defaults" everywhere. Optional features register their own
 * sections.
 */
#include <string.h>

#include "profile/profile.h"

#include "analog/analog_data.h"
#include "core/clock.h"
#include "hid/hid.h"
#include "keys/keys.h"
#include "layout/layout.h"
#include "protocol/protocol.h"
#include "rgb/rgb_data.h"
#include "rgb/rgb_effects.h"
#include "rgb/rgb_frame.h"
#include "storage/storage.h"
#include "usb/usb_data.h"

#define PROFILE_PATH_MAX    16u

int profile_read_pb_file(uint16_t profile_id, const char *name, const pb_msgdesc_t *msgdesc, void *msg)
{
    char path[PROFILE_PATH_MAX];

    if (g_storage_mounted == 0u) {
        return STORAGE_ERR_NOT_MOUNTED;
    }
    storage_format_profile_path(path, profile_id, name);
    return storage_read_pb_file(path, msgdesc, msg);
}

/* Forgets the states of the Fn key actions of the previous profile. */
static void profile_reset_fn_actions(uint16_t profile_id, uint32_t default_index)
{
    memset(g_keys_fn_action_states, 0, sizeof(g_keys_fn_action_states));
}
PROFILE_SECTION(PROFILE_SECTION_FN_ACTIONS, profile_reset_fn_actions);

/* File "cfg" (with fallbacks) -> the live settings. */
static void profile_load_cfg(uint16_t profile_id, uint32_t default_index)
{
    profile_load_and_apply_cfg(profile_id);
}
PROFILE_SECTION(PROFILE_SECTION_CONFIG, profile_load_cfg);

/* File "rgb" -> g_rgb_settings; factory defaults if it cannot be read. */
static void profile_load_rgb_settings(uint16_t profile_id, uint32_t default_index)
{
    if (profile_read_pb_file(profile_id, "rgb", &RgbSettings_msg, &g_rgb_settings) != STORAGE_OK) {
        profile_fill_default_rgb_settings(default_index, &g_rgb_settings);
    }
}
PROFILE_SECTION(PROFILE_SECTION_LIGHTING_SETTINGS, profile_load_rgb_settings);

/* File "clr" -> g_rgb_base_colors; factory defaults if it cannot be read. */
static void profile_load_base_colors(uint16_t profile_id, uint32_t default_index)
{
    char path[PROFILE_PATH_MAX];
    bool loaded = false;

    if (g_storage_mounted != 0u) {
        storage_format_profile_path(path, profile_id, "clr");
        int length = storage_read_file(path, g_rgb_base_colors, STORAGE_BASE_COLORS_SIZE);
        /* A negative result whose low 16 bits are 1 is also accepted. Matches stock firmware 2.14.1. */
        loaded = length >= 0 || (int16_t)length == STORAGE_OK;
    }
    if (!loaded) {
        profile_copy_default_base_colors(default_index, &g_rgb_base_colors[0][0]);
    }
}
PROFILE_SECTION(PROFILE_SECTION_BASE_COLORS, profile_load_base_colors);

/* Sets up the effect layers from the loaded RGB settings. */
static void profile_init_rgb_effects(uint16_t profile_id, uint32_t default_index)
{
    if (g_rgb_settings.which_effect == RgbSettings_legacy_effect_tag) {
        rgb_settings_convert_legacy_effect();
        rgb_effects_apply_layer_settings();
    } else if (g_rgb_settings.which_effect == RgbSettings_layers_tag) {
        rgb_effects_apply_layer_settings();
    } else {
        g_rgb_settings.effect.layers.layers_count = 0u;
        g_rgb_effect_layer_count = 0u;
    }
}
PROFILE_SECTION(PROFILE_SECTION_EFFECT_START, profile_init_rgb_effects);

/* Files "r1".."r3" -> g_rgb_layer_colors, up to the first missing one. */
static void profile_load_layer_colors(uint16_t profile_id, uint32_t default_index)
{
    g_rgb_layer_color_count = 0u;
    for (uint8_t layer = 1u; layer <= RGB_COLOR_LAYER_COUNT; layer++) {
        if (storage_read_layer_colors(profile_id, layer, g_rgb_layer_colors[layer - 1u]) != STORAGE_OK) {
            break;
        }
        g_rgb_layer_color_count++;
    }
}
PROFILE_SECTION(PROFILE_SECTION_LAYER_COLORS, profile_load_layer_colors);

/* Shows colour layer 0 again, forces a full render and stops the host light bar overlay. */
static void profile_reset_rgb_frame(uint16_t profile_id, uint32_t default_index)
{
    g_rgb_layer_switch_pending = 0u;
    g_rgb_shown_color_layer = 0u;
    rgb_paint_base_colors(0u);
    g_rgb_force_render = 1u;
    rgb_update_lock_indicators();
    rgb_overlay_stop(RGB_OVERLAY_ID_HOST_LIGHTBAR);
}
PROFILE_SECTION(PROFILE_SECTION_FRAME_RESET, profile_reset_rgb_frame);

/* File "act" -> per-key actuation points; without the file every key uses the profile value. */
static void profile_load_actuation_points(uint16_t profile_id, uint32_t default_index)
{
    ActuationPoints act;

    if (profile_read_pb_file(profile_id, "act", &ActuationPoints_msg, &act) == STORAGE_OK) {
        keys_sanitize_actuation_map(&act);
        keys_apply_actuation_map(&act);
        return;
    }
    for (uint32_t i = 0u; i != ANALOG_KEY_COUNT; i++) {
        g_analog_key_states[i].actuation_point = 0u;
    }
}
PROFILE_SECTION(PROFILE_SECTION_ACTUATION_POINTS, profile_load_actuation_points);

/* Per-key rapid trigger: legacy file "prt", else the groups of "prt2". */
static void profile_load_rapid_trigger(uint16_t profile_id, uint32_t default_index)
{
    KeyByteMap legacy_keys;

    g_keys_rt_groups_active = 0u;
    for (uint32_t i = 0u; i != ANALOG_KEY_COUNT; i++) {
        g_analog_key_states[i].flags &= (uint8_t)~(ANALOG_KEY_FLAG_RT_OVERRIDE | ANALOG_KEY_FLAG_RT_ON);
        g_analog_key_states[i].rt_params = NULL;
    }

    if (g_storage_mounted == 0u) {
        return;
    }
    if (profile_read_pb_file(profile_id, "prt", &KeyByteMap_msg, &legacy_keys) == STORAGE_OK) {
        keys_apply_legacy_rapid_trigger_keys(&legacy_keys);
    } else if (profile_read_pb_file(profile_id, "prt2", &RapidTriggerGroups_msg, &g_keys_rt_groups) == STORAGE_OK) {
        keys_sanitize_rapid_trigger_groups(&g_keys_rt_groups);
        g_keys_rt_groups_active = 1u;
        keys_apply_rapid_trigger_groups();
    }
}
PROFILE_SECTION(PROFILE_SECTION_RAPID_TRIGGER, profile_load_rapid_trigger);

/* Key map layers of the profile (layout_load_keymap_layers). */
static void profile_load_key_maps(uint16_t profile_id, uint32_t default_index)
{
    layout_load_keymap_layers(profile_id);
}
PROFILE_SECTION(PROFILE_SECTION_KEY_MAPS, profile_load_key_maps);

/* File "ak2" -> g_keys_advanced_keys, bound to the keys. */
static void profile_load_advanced_keys(uint16_t profile_id, uint32_t default_index)
{
    keys_stop_toggle_overlays();
    memset(g_keys_advanced_key_states, 0, sizeof(g_keys_advanced_key_states));

    if (profile_read_pb_file(profile_id, "ak2", &AdvancedKeys_msg, &g_keys_advanced_keys) != STORAGE_OK) {
        g_keys_advanced_keys.entries_count = 0u;
    }
    keys_validate_advanced_keys(&g_keys_advanced_keys);

    for (uint32_t i = 0u; i != ANALOG_KEY_COUNT; i++) {
        memset(g_analog_key_states[i].ak_slot, 0xFF, sizeof(g_analog_key_states[i].ak_slot));
    }
    for (uint8_t i = 0u; i < g_keys_advanced_keys.entries_count; i++) {
        keys_apply_advanced_key_entry(i, &g_keys_advanced_keys.entries[i]);
    }
}
PROFILE_SECTION(PROFILE_SECTION_ADVANCED_KEYS, profile_load_advanced_keys);

/* Restarts the time base of the key pass. */
static void profile_reset_key_task_stamp(uint16_t profile_id, uint32_t default_index)
{
    g_keys_task_stamp_ms = g_clock_uptime_ms;
}
PROFILE_SECTION(PROFILE_SECTION_TIME_STAMP, profile_reset_key_task_stamp);

/* Neutral gamepad report for the active USB mode, so that no axis or button
 * of the previous profile stays held. */
static void profile_send_neutral_gamepad_report(uint16_t profile_id, uint32_t default_index)
{
    uint8_t usb_mode = g_usb_link_flags & USB_LINK_MODE_MASK;

    if (usb_mode == UsbGamepadMode_USB_GAMEPAD_MODE_XINPUT) {
        hid_xinput_pad_state_t pad_state;

        memset(&pad_state, 0, sizeof(pad_state));
        if (g_hid_xinput_report_enabled != 0u) {
            hid_send_xinput_report(&pad_state);
        }
    } else if (usb_mode == UsbGamepadMode_USB_GAMEPAD_MODE_HID) {
        hid_classic_gamepad_report_t report;

        memset(&report, 0, sizeof(report));
        report.x = HID_GAMEPAD_AXIS_CENTER;
        report.y = HID_GAMEPAD_AXIS_CENTER;
        report.rx = HID_GAMEPAD_AXIS_CENTER;
        report.ry = HID_GAMEPAD_AXIS_CENTER;
        report.extra[0] = HID_GAMEPAD_AXIS_CENTER;
        report.extra[1] = HID_GAMEPAD_AXIS_CENTER;
        hid_send_gamepad_report(&report);
    }
}
PROFILE_SECTION(PROFILE_SECTION_NEUTRAL_GAMEPAD_REPORT, profile_send_neutral_gamepad_report);

/* Files "gp" -> g_hid_gamepad_settings and "gm" -> per-key gamepad bindings. */
static void profile_load_gamepad(uint16_t profile_id, uint32_t default_index)
{
    GamepadMapping mapping;
    bool mapping_loaded = false;

    if (g_storage_mounted != 0u) {
        if (profile_read_pb_file(profile_id, "gp", &GamepadSettings_msg, &g_hid_gamepad_settings) == STORAGE_OK) {
            g_hid_gamepad_file_flags |= HID_GAMEPAD_FILE_GP_LOADED;
        } else {
            g_hid_gamepad_file_flags &= (uint8_t)~HID_GAMEPAD_FILE_GP_LOADED;
        }
        memset(&mapping, 0, sizeof(mapping));
        mapping_loaded = profile_read_pb_file(profile_id, "gm", &GamepadMapping_msg, &mapping) == STORAGE_OK;
    } else {
        g_hid_gamepad_file_flags &= (uint8_t)~HID_GAMEPAD_FILE_GP_LOADED;
        memset(&mapping, 0, sizeof(mapping));
    }

    if (mapping_loaded) {
        g_hid_gamepad_file_flags |= HID_GAMEPAD_FILE_GM_LOADED;
    } else {
        g_hid_gamepad_file_flags &= (uint8_t)~HID_GAMEPAD_FILE_GM_LOADED;
    }
    keys_apply_gamepad_map(&mapping);
}
PROFILE_SECTION(PROFILE_SECTION_GAMEPAD, profile_load_gamepad);

/* Sends the "profile activated" event to the host if it subscribed to it. */
static void profile_send_activated_event(uint16_t profile_id, uint32_t default_index)
{
    Event event;

    memset(&event, 0, sizeof(event));
    event.which_event = Event_profile_activated_tag;
    event.event.profile_activated.has_profile_namespace = true;
    event.event.profile_activated.profile_index = PROFILE_ID_INDEX(profile_id);
    event.event.profile_activated.profile_namespace = PROFILE_ID_NS(profile_id);
    if (g_protocol_event_subscriptions[PROTOCOL_EVENT_PROFILE_ACTIVATED] != 0u) {
        protocol_send_event(&event);
    }
}
PROFILE_SECTION(PROFILE_SECTION_ACTIVATED_EVENT, profile_send_activated_event);
