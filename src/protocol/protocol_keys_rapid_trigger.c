/*
 * protocol_keys_rapid_trigger.c - host commands for the per-key rapid-trigger settings.
 *
 * Two generations exist: the legacy per-key mode map (profile file "prt", feature command 54 /
 * OUT-report command 25) and the group list (profile file "prt2", feature command 61 /
 * OUT-report command 29). g_keys_rt_groups_active tells which one the RAM state came from.
 */
#include <string.h>

#include "pb_decode.h"
#include "pb_encode.h"

#include "analog/analog.h"
#include "keys/keys.h"
#include "layout/layout.h"
#include "profile/profile.h"
#include "proto/settings.pb.h"
#include "protocol/protocol.h"
#include "protocol/protocol_keys_internal.h"
#include "storage/storage.h"

/* Rebuilds the legacy message from the key flags. Returns false when there is nothing to
 * report (sparse form without entries). */
static bool protocol_export_legacy_rapid_trigger_keys(KeyByteMap *modes)
{
    uint8_t override_count = g_keys_rt_legacy_override_count;
    bool dense = override_count > KEYS_LEGACY_MAP_THRESHOLD;

    /* KNOWN ISSUE: the message is not cleared. In the dense form the cells of keys without an
     * override (and columns 17..20) keep whatever was on the stack. Matches stock
     * firmware 2.14.1. */
    if (dense) {
        modes->which_map = KeyByteMap_matrix_tag;
    } else {
        modes->which_map = KeyByteMap_list_tag;
        modes->map.list.items_count = 0;
    }

    for (uint32_t row = 0; row != LAYOUT_ROWS; ++row) {
        for (uint32_t col = 0; col != LAYOUT_COLS; ++col) {
            uint8_t pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];
            if (pos == LAYOUT_NO_KEY) {
                continue;
            }
            uint8_t flags = g_analog_key_states[pos].flags;
            if (!(flags & ANALOG_KEY_FLAG_RT_OVERRIDE)) {
                continue;
            }
            bool rapid_trigger_on = (flags & ANALOG_KEY_FLAG_RT_ON) != 0;

            if (dense) {
                modes->map.matrix.rows[row].values[col] =
                    rapid_trigger_on ? KEYS_RT_LEGACY_ON : KEYS_RT_LEGACY_OFF;
            } else {
                /* Sparse items carry the bare on / off bit, not enum keys_rt_legacy_mode. */
                KeyValueListU8 *list = &modes->map.list;

                list->items[list->items_count] =
                    (uint16_t)(KEYS_KEY_ID(row, col) | ((uint16_t)rapid_trigger_on << 8));
                list->items_count++;
            }
        }
    }

    return dense || modes->map.list.items_count != 0;
}

/* Feature command 54. arg bits 15:0 = profile id, 0xFFFF = the live state. Reply: the
 * KeyByteMap message with the per-key rapid-trigger modes (from the profile file "prt" for a
 * stored profile); empty payload if the group list is in use, nothing is set or the file
 * does not exist. */
static void protocol_cmd_get_legacy_rapid_trigger_keys(uint32_t arg, protocol_response_t *response)
{
    uint16_t profile_id = (uint16_t)arg;
    KeyByteMap modes;

    if (profile_id == PROFILE_ID_NONE) {
        if (g_keys_rt_groups_active != 0 || !protocol_export_legacy_rapid_trigger_keys(&modes)) {
            protocol_reply_empty(response);
            return;
        }
    } else {
        char path[PROTOCOL_PROFILE_PATH_SIZE];
        int result;

        if (g_storage_mounted == 0) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
        storage_format_profile_path(path, profile_id, "prt");
        result = storage_read_pb_file(path, &KeyByteMap_msg, &modes);
        if (result == LFS_ERR_NOENT) {
            protocol_reply_empty(response);
            return;
        }
        if (result != 1) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
    }

    protocol_reply_message(response, &KeyByteMap_msg, &modes);
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_LEGACY_RAPID_TRIGGER_KEYS, protocol_cmd_get_legacy_rapid_trigger_keys);

/* OUT-report command 25. Payload: KeyByteMap message with the per-key rapid-trigger modes;
 * an empty payload removes every per-key override. With the persist flag the profile file
 * "prt" is written (or removed for an empty payload); the RAM state changes for a live
 * preview or when the active profile is addressed. No response. */
static void protocol_cmd_set_legacy_rapid_trigger_keys(const uint8_t *request)
{
    const protocol_report_profile_t *report = (const protocol_report_profile_t *)request;
    uint16_t length = report->length;
    uint16_t profile_id;
    KeyByteMap modes;

    if (length != 0) {
        pb_istream_t stream = pb_istream_from_buffer(report->payload, length);

        if (!pb_decode(&stream, &KeyByteMap_msg, &modes)) {
            return;
        }
        profile_id = protocol_report_profile_id(report->flags);
        if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
            storage_write_profile_pb_file(profile_id, "prt", &KeyByteMap_msg, KeyByteMap_size, &modes);
        }
    } else {
        profile_id = protocol_report_profile_id(report->flags);
        if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
            char path[PROTOCOL_PROFILE_PATH_SIZE];

            storage_format_profile_path(path, profile_id, "prt");
            lfs_remove(&g_storage_lfs, path);
            storage_bump_profile_revision(profile_id);
        }
    }

    if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && !protocol_profile_is_active(profile_id)) {
        return;
    }

    g_keys_rt_groups_active = 0;
    if (length != 0) {
        keys_apply_legacy_rapid_trigger_keys(&modes);
    } else {
        for (uint32_t pos = 0; pos != ANALOG_KEY_COUNT; ++pos) {
            analog_key_state_t *key = &g_analog_key_states[pos];

            key->flags &= (uint8_t)~(ANALOG_KEY_FLAG_RT_OVERRIDE | ANALOG_KEY_FLAG_RT_ON);
            key->rt_params = NULL;
        }
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_LEGACY_RAPID_TRIGGER_KEYS, protocol_cmd_set_legacy_rapid_trigger_keys);

/* Feature command 61. arg bits 15:0 = profile id, 0xFFFF = the live state. Reply: the
 * RapidTriggerGroups message (from the profile file "prt2" for a stored profile); empty
 * payload if no group list is in use or the file does not exist. */
static void protocol_cmd_get_rapid_trigger_groups(uint32_t arg, protocol_response_t *response)
{
    uint16_t profile_id = (uint16_t)arg;
    RapidTriggerGroups groups = RapidTriggerGroups_init_default;  /* zero, sensitivity 65 in all 30 groups */
    char path[PROTOCOL_PROFILE_PATH_SIZE];
    int result;

    if (profile_id == PROFILE_ID_NONE) {
        if (g_keys_rt_groups_active != 0 && g_keys_rt_groups.groups_count != 0) {
            protocol_reply_message(response, &RapidTriggerGroups_msg, &g_keys_rt_groups);
        } else {
            protocol_reply_empty(response);
        }
        return;
    }

    if (g_storage_mounted == 0) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }
    storage_format_profile_path(path, profile_id, "prt2");
    result = storage_read_pb_file(path, &RapidTriggerGroups_msg, &groups);
    if (result == 1) {
        keys_sanitize_rapid_trigger_groups(&groups);
        protocol_reply_message(response, &RapidTriggerGroups_msg, &groups);
    } else if (result == LFS_ERR_NOENT) {
        protocol_reply_empty(response);
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_RAPID_TRIGGER_GROUPS, protocol_cmd_get_rapid_trigger_groups);

/* OUT-report command 29. Payload: RapidTriggerGroups message; an empty payload removes all
 * groups. With the persist flag the profile file "prt2" is written and the legacy file "prt"
 * removed (both removed for an empty payload); the RAM state changes for a live preview or
 * when the active profile is addressed. No response. */
static void protocol_cmd_set_rapid_trigger_groups(const uint8_t *request)
{
    const protocol_report_profile_t *report = (const protocol_report_profile_t *)request;
    uint16_t length = report->length;
    uint16_t profile_id = protocol_report_profile_id(report->flags);
    char path[PROTOCOL_PROFILE_PATH_SIZE];
    RapidTriggerGroups groups;

    if (length != 0) {
        pb_istream_t stream = pb_istream_from_buffer(report->payload, length);

        if (!pb_decode(&stream, &RapidTriggerGroups_msg, &groups)) {
            return;
        }
        keys_sanitize_rapid_trigger_groups(&groups);

        if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
            /* The group list replaces the legacy file. */
            storage_format_profile_path(path, profile_id, "prt");
            lfs_remove(&g_storage_lfs, path);
            storage_bump_profile_revision(profile_id);
            storage_write_profile_pb_file(profile_id, "prt2", &RapidTriggerGroups_msg, RapidTriggerGroups_size,
                                          &groups);
        }
    } else if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
        storage_format_profile_path(path, profile_id, "prt");
        lfs_remove(&g_storage_lfs, path);
        storage_bump_profile_revision(profile_id);
        storage_format_profile_path(path, profile_id, "prt2");
        lfs_remove(&g_storage_lfs, path);
        storage_bump_profile_revision(profile_id);
    }

    if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && !protocol_profile_is_active(profile_id)) {
        return;
    }

    g_keys_rt_groups_active = 1;
    if (length != 0) {
        memcpy(&g_keys_rt_groups, &groups, sizeof(g_keys_rt_groups));
    } else {
        g_keys_rt_groups.groups_count = 0;
    }
    keys_apply_rapid_trigger_groups();
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_RAPID_TRIGGER_GROUPS, protocol_cmd_set_rapid_trigger_groups);
