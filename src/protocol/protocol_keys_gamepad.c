/*
 * protocol_keys_gamepad.c - host commands for the gamepad function of the keys.
 *
 * Per-key gamepad bindings (profile file "gm", feature command 40 / OUT-report command 18) and
 * the stick response settings (profile file "gp", feature command 41 / OUT-report command 19).
 */
#include <string.h>

#include "pb_decode.h"
#include "pb_encode.h"

#include "analog/analog.h"
#include "hid/hid.h"
#include "keys/keys.h"
#include "layout/layout.h"
#include "profile/profile.h"
#include "proto/settings.pb.h"
#include "protocol/protocol.h"
#include "protocol/protocol_keys_internal.h"
#include "storage/storage.h"

/* OUT-report command 18. Payload: GamepadMapping message (per-key gamepad bindings); an
 * empty payload removes all bindings. With the persist flag the profile file "gm" is written
 * (or removed for an empty payload); the RAM state changes for a live preview or when the
 * active profile is addressed. No response. */
static void protocol_cmd_set_gamepad_mapping(const uint8_t *request)
{
    const protocol_report_profile_t *report = (const protocol_report_profile_t *)request;
    uint16_t length = report->length;
    uint16_t profile_id = protocol_report_profile_id(report->flags);
    GamepadMapping mapping;

    memset(&mapping, 0, sizeof(mapping));

    if (length != 0) {
        pb_istream_t stream = pb_istream_from_buffer(report->payload, length);

        if (!pb_decode(&stream, &GamepadMapping_msg, &mapping)) {
            return;
        }
        if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
            storage_write_profile_pb_file(profile_id, "gm", &GamepadMapping_msg, GamepadMapping_size, &mapping);
        }
    } else if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
        char path[PROTOCOL_PROFILE_PATH_SIZE];

        storage_format_profile_path(path, profile_id, "gm");
        lfs_remove(&g_storage_lfs, path);
        storage_bump_profile_revision(profile_id);
    }

    if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && !protocol_profile_is_active(profile_id)) {
        return;
    }

    /* An empty request applies the zeroed message: no bindings. */
    keys_apply_gamepad_map((const void *)&mapping);
    g_hid_gamepad_file_flags =
        (uint8_t)((g_hid_gamepad_file_flags & ~HID_GAMEPAD_FILE_GM_LOADED) | (length != 0));
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_GAMEPAD_MAPPING, protocol_cmd_set_gamepad_mapping);

/* Feature command 40. arg bits 15:0 = profile id, 0xFFFF = the live bindings. Reply: the
 * GamepadMapping message (from the profile file "gm" for a stored profile); empty payload
 * if no mapping is loaded or the file does not exist. */
static void protocol_cmd_get_gamepad_mapping(uint32_t arg, protocol_response_t *response)
{
    uint16_t profile_id = (uint16_t)arg;
    GamepadMapping mapping;

    memset(&mapping, 0, sizeof(mapping));

    if (profile_id == PROFILE_ID_NONE) {
        if (!(g_hid_gamepad_file_flags & HID_GAMEPAD_FILE_GM_LOADED)) {
            protocol_reply_empty(response);
            return;
        }

        uint8_t mapped_count = g_hid_gamepad_mapped_key_count;
        bool dense = mapped_count > KEYS_LEGACY_MAP_THRESHOLD;

        mapping.gamepad_keys_exclusive = g_hid_gamepad_keys_exclusive;
        mapping.bindings.which_map = dense ? KeyByteMap_matrix_tag : KeyByteMap_list_tag;

        for (uint32_t row = 0; row != LAYOUT_ROWS; ++row) {
            for (uint32_t col = 0; col != LAYOUT_COLS; ++col) {
                uint8_t pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];
                if (pos == LAYOUT_NO_KEY) {
                    continue;
                }
                uint8_t binding = g_analog_key_states[pos].gamepad_binding;
                if (binding == 0) {
                    continue;
                }

                if (dense) {
                    mapping.bindings.map.matrix.rows[row].values[col] = binding;
                } else {
                    KeyValueListU8 *list = &mapping.bindings.map.list;

                    list->items[list->items_count] = (uint16_t)(KEYS_KEY_ID(row, col) | ((uint16_t)binding << 8));
                    list->items_count++;
                }
            }
        }
    } else {
        char path[PROTOCOL_PROFILE_PATH_SIZE];
        int result;

        if (g_storage_mounted == 0) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
        storage_format_profile_path(path, profile_id, "gm");
        result = storage_read_pb_file(path, &GamepadMapping_msg, &mapping);
        if (result == LFS_ERR_NOENT) {
            protocol_reply_empty(response);
            return;
        }
        if (result != 1) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
    }

    protocol_reply_message(response, &GamepadMapping_msg, &mapping);
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_GAMEPAD_MAPPING, protocol_cmd_get_gamepad_mapping);

/* OUT-report command 19. Payload: GamepadSettings message (stick response settings); an
 * empty payload restores the defaults. With the persist flag the profile file "gp" is
 * written; the RAM state changes for a live preview or when the active profile is
 * addressed. No response. */
static void protocol_cmd_set_gamepad_settings(const uint8_t *request)
{
    const protocol_report_profile_t *report = (const protocol_report_profile_t *)request;
    uint16_t length = report->length;
    uint16_t profile_id = protocol_report_profile_id(report->flags);
    GamepadSettings settings = GamepadSettings_init_default;  /* all zero, left_stick_up_x_percent 50 */

    if (length != 0) {
        pb_istream_t stream = pb_istream_from_buffer(report->payload, length);

        if (!pb_decode(&stream, &GamepadSettings_msg, &settings)) {
            return;
        }
        if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
            storage_write_profile_pb_file(profile_id, "gp", &GamepadSettings_msg, GamepadSettings_size, &settings);
        }
    } else if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
        char path[PROTOCOL_PROFILE_PATH_SIZE];

        /* KNOWN ISSUE: resetting the settings removes the mapping file "gm" instead of "gp";
         * a stored "gp" file survives the reset. Matches stock firmware 2.14.1. */
        storage_format_profile_path(path, profile_id, "gm");
        lfs_remove(&g_storage_lfs, path);
        storage_bump_profile_revision(profile_id);
    }

    if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && !protocol_profile_is_active(profile_id)) {
        return;
    }

    g_hid_gamepad_settings = settings;
    g_hid_gamepad_file_flags =
        (uint8_t)((g_hid_gamepad_file_flags & ~HID_GAMEPAD_FILE_GP_LOADED) | (length != 0 ? HID_GAMEPAD_FILE_GP_LOADED : 0u));
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_GAMEPAD_SETTINGS, protocol_cmd_set_gamepad_settings);

/* Feature command 41. arg bits 15:0 = profile id, 0xFFFF = the live settings. Reply: the
 * GamepadSettings message (from the profile file "gp" for a stored profile); empty payload
 * if no settings are loaded or the file does not exist. */
static void protocol_cmd_get_gamepad_settings(uint32_t arg, protocol_response_t *response)
{
    uint16_t profile_id = (uint16_t)arg;

    if (profile_id == PROFILE_ID_NONE) {
        if (g_hid_gamepad_file_flags & HID_GAMEPAD_FILE_GP_LOADED) {
            protocol_reply_message(response, &GamepadSettings_msg, &g_hid_gamepad_settings);
        } else {
            protocol_reply_empty(response);
        }
        return;
    }

    GamepadSettings settings = GamepadSettings_init_default;
    char path[PROTOCOL_PROFILE_PATH_SIZE];
    int result;

    if (g_storage_mounted == 0) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }
    storage_format_profile_path(path, profile_id, "gp");
    result = storage_read_pb_file(path, &GamepadSettings_msg, &settings);
    if (result == 1) {
        protocol_reply_message(response, &GamepadSettings_msg, &settings);
    } else if (result == LFS_ERR_NOENT) {
        protocol_reply_empty(response);
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_GAMEPAD_SETTINGS, protocol_cmd_get_gamepad_settings);
