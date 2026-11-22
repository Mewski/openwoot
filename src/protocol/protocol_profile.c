/*
 * protocol_profile.c - profile commands of the host protocol: selecting, reloading,
 * creating and deleting profiles, the general profile settings (file "cfg"), the profile
 * info record (file "i") and the revision counter (file "pst").
 *
 * A profile id travels as 16 bits: low byte index, high byte namespace (profile_id_t).
 * OUT reports address a profile through the flags byte (PROTOCOL_REPORT_FLAG_*).
 */
#include <stdio.h>
#include <string.h>

#include "lfs.h"
#include "pb_decode.h"
#include "pb_encode.h"

#include "protocol/protocol.h"
#include "profile/profile.h"
#include "rgb/rgb_frame.h"
#include "rgb/signals/signals.h"
#include "storage/storage.h"

#define PROTOCOL_EINVAL            22u   /* error payload of a bad profile id */
#define PROTOCOL_PROFILE_PATH_MAX  12u   /* "lp/<n>/<name>" buffers of the handlers */

/* Encoded size limits passed to storage_write_profile_pb_file. */
#define PROTOCOL_CFG_FILE_MAX      26u
#define PROTOCOL_INFO_FILE_MAX     65u

/* Index range of a profile directory: 0..3 in the built-in namespace and 0..9 in "lp". */
static bool protocol_profile_slot_is_valid(uint8_t index, uint8_t ns)
{
    if (ns == PROFILE_NS_BUILTIN) {
        return index < PROFILE_BUILTIN_MAX;
    }
    return ns == PROFILE_NS_LP && index < PROFILE_LP_MAX;
}

/* Asks the main loop to activate the given profile. */
static void protocol_request_profile_switch(profile_id_t target)
{
    g_profile_switch_pending = 1u;
    g_profile_switch_target = target;
}

/* Error reply of the directory commands: one byte, the positive error number. */
static void protocol_respond_error_byte(protocol_response_t *response, uint8_t error)
{
    response->status = PROTOCOL_STATUS_ERROR;
    response->length = 1u;
    response->payload[0] = error;
}

/* ---- Active profile ---------------------------------------------------------------------------- */

/* Feature command 7. Reloads the active profile from storage; the reload is executed by the
 * main loop. Reply: status OK, no payload. */
static void protocol_cmd_reload_profile_legacy(uint32_t arg, protocol_response_t *response)
{
    protocol_request_profile_switch(g_profile_active_id);
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_RELOAD_PROFILE_LEGACY, protocol_cmd_reload_profile_legacy);

/* Feature command 38, same as command 7. */
static void protocol_cmd_reload_profile(uint32_t arg, protocol_response_t *response)
{
    protocol_request_profile_switch(g_profile_active_id);
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_RELOAD_PROFILE, protocol_cmd_reload_profile);

/* Feature command 11. Reply: two bytes, index and namespace of the active profile. */
static void protocol_cmd_get_active_profile(uint32_t arg, protocol_response_t *response)
{
    response->payload[1] = g_profile_active_id.ns;
    response->payload[0] = g_profile_active_id.index;
    response->length = 2u;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_ACTIVE_PROFILE, protocol_cmd_get_active_profile);

/* Feature command 23. arg bits 15:0 = profile id. Activates an existing profile (executed by
 * the main loop). Reply: status OK, or 0xFF if the profile does not exist. */
static void protocol_cmd_activate_profile(uint32_t arg, protocol_response_t *response)
{
    profile_id_t target = { .raw = (uint16_t)arg };

    if (target.ns < PROFILE_NS_COUNT && target.index < g_profile_counts[target.ns]) {
        protocol_request_profile_switch(target);
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_ACTIVATE_PROFILE, protocol_cmd_activate_profile);

/* Feature command 73. Reply: ActiveProfileState message (index and namespace of the active
 * profile, SDK colour mode active, LEDs switched off by the idle timer). */
static void protocol_cmd_get_active_profile_state(uint32_t arg, protocol_response_t *response)
{
    ActiveProfileState state;

    state.profile_index = g_profile_active_id.index;
    state.has_sdk_mode_active = true;
    state.has_profile_namespace = true;
    state.profile_namespace = g_profile_active_id.ns;
    state.has_leds_off_by_idle = true;
    state.sdk_mode_active = g_rgb_sdk_mode_active;
    state.leds_off_by_idle = (g_rgb_idle_flags & RGB_IDLE_FLAG_OFF) != 0u;

    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);

    if (pb_encode(&stream, &ActiveProfileState_msg, &state)) {
        response->length = (uint16_t)stream.bytes_written;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_ACTIVE_PROFILE_STATE, protocol_cmd_get_active_profile_state);

/* Feature command 84. The host selects which profile of the "lp" namespace
 * the keyboard may enter; id 0xFFFF clears the selection. arg bits 15:0 = profile id,
 * arg byte 2 = switch now. Reply: no payload; status 0xFF for an id that does not exist. */
static void protocol_cmd_select_lp_profile(uint32_t arg, protocol_response_t *response)
{
    profile_id_t id = { .raw = (uint16_t)arg };

    if (id.raw == PROFILE_ID_NONE) {
        if (g_profile_active_id.ns == PROFILE_NS_LP) {
            protocol_request_profile_switch(g_profile_lp_return_id);
        }
        g_profile_lp_selected_id.raw = 0u;
        g_profile_lp_selected = 0u;
        response->status = PROTOCOL_STATUS_OK;
        response->length = 0u;
        return;
    }

    if (id.ns != PROFILE_NS_LP || id.index >= PROFILE_LP_MAX
        || id.index >= g_profile_counts[PROFILE_COUNTS_LP]) {
        response->status = PROTOCOL_STATUS_ERROR;
        response->length = 0u;
        return;
    }

    g_profile_lp_selected = 1u;
    g_profile_lp_selected_id.ns = PROFILE_NS_LP;
    g_profile_lp_selected_id.index = id.index;

    if (g_profile_active_id.ns == PROFILE_NS_LP || (arg & 0x00FF0000u) != 0u) {
        g_profile_switch_pending = 1u;
        g_profile_switch_target.index = id.index;
        g_profile_switch_target.ns = PROFILE_NS_LP;
    }
    response->status = PROTOCOL_STATUS_OK;
    response->length = 0u;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_SELECT_LP_PROFILE, protocol_cmd_select_lp_profile);

/* Feature command 85. Reply: two bytes, index and namespace of the profile selected with
 * command 84; no payload if none is selected. */
static void protocol_cmd_get_lp_profile_selection(uint32_t arg, protocol_response_t *response)
{
    if (g_profile_lp_selected != 0u) {
        profile_id_t selected = g_profile_lp_selected_id;

        response->status = PROTOCOL_STATUS_OK;
        response->length = 2u;
        response->payload[0] = selected.index;
        response->payload[1] = selected.ns;
    } else {
        response->status = PROTOCOL_STATUS_OK;
        response->length = 0u;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_LP_PROFILE_SELECTION, protocol_cmd_get_lp_profile_selection);

/* ---- Profile directories ----------------------------------------------------------------------- */

/* Feature command 62. arg byte 0 = namespace. Reply: one byte, the number of profile
 * directories; on failure status 0xFF and one byte with the error number. */
static void protocol_cmd_get_profile_count(uint32_t arg, protocol_response_t *response)
{
    uint8_t ns = (uint8_t)arg;
    uint8_t count;

    if (ns >= PROFILE_NS_COUNT) {
        protocol_respond_error_byte(response, PROTOCOL_EINVAL);
        return;
    }

    int result = storage_count_profile_dirs(ns, &count);

    if (result == STORAGE_OK) {
        response->length = 1u;
        response->status = PROTOCOL_STATUS_OK;
        response->payload[0] = count;
    } else {
        protocol_respond_error_byte(response, (uint8_t)-result);
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_PROFILE_COUNT, protocol_cmd_get_profile_count);

/* Creates the directory of a profile ("lp" first for that namespace; the result of that
 * mkdir is ignored). Returns STORAGE_OK or the negative error truncated to 16 bits. */
static int16_t protocol_create_profile_dir(uint8_t index, uint8_t ns)
{
    char path[8];

    if (g_storage_mounted == 0u) {
        return STORAGE_ERR_NOT_MOUNTED;
    }

    if (ns == PROFILE_NS_LP) {
        lfs_mkdir(&g_storage_lfs, "lp");
        sprintf(path, "lp/%d", index);
    } else {
        sprintf(path, "%d", index);
    }

    int err = lfs_mkdir(&g_storage_lfs, path);

    return (err < 0) ? (int16_t)err : (int16_t)STORAGE_OK;
}

/* Feature command 64. arg bits 15:0 = profile id. Only the directory is created, no files;
 * the profile count is updated. Reply: status OK, or 0xFF (with one error byte if the
 * directory could not be created). */
static void protocol_cmd_create_profile(uint32_t arg, protocol_response_t *response)
{
    profile_id_t id = { .raw = (uint16_t)arg };

    if (id.raw == PROFILE_ID_NONE || !protocol_profile_slot_is_valid(id.index, id.ns)) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    int16_t result = protocol_create_profile_dir(id.index, id.ns);

    if (result != STORAGE_OK) {
        protocol_respond_error_byte(response, (uint8_t)-result);
        return;
    }
    response->status = PROTOCOL_STATUS_OK;

    uint8_t count;

    if (storage_count_profile_dirs(id.ns, &count) == STORAGE_OK) {
        g_profile_counts[id.ns] = count;
        if (id.ns == PROFILE_NS_BUILTIN) {
            signals_publish_if_changed(RGB_SIGNAL_PROFILE_COUNT, (float)g_profile_counts[PROFILE_COUNTS_BUILTIN]);
        }
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_CREATE_PROFILE, protocol_cmd_create_profile);

/* Feature command 63. arg bits 15:0 = profile id. Deletes the last profile of a namespace,
 * see protocol_delete_last_profile. */
static void protocol_cmd_delete_profile(uint32_t arg, protocol_response_t *response)
{
    if ((uint16_t)arg == PROFILE_ID_NONE) {
        response->status = PROTOCOL_STATUS_ERROR;
    } else {
        protocol_delete_last_profile(arg, response, PROFILE_ID_NONE, (uint16_t)arg);
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_DELETE_PROFILE, protocol_cmd_delete_profile);

/* Body of feature command 63. arg is the whole argument word and is reduced to the index
 * byte here, profile_id is the 16-bit profile id, the third argument is not used. Only the
 * last profile of a namespace can be deleted, and never built-in profile 0. Afterwards the
 * profile counts, the active profile and the stored profile references are adjusted and the
 * "profiles changed" event is requested. Reply: status OK, or 0xFF (with one error byte for
 * a profile that may not be deleted or a storage error). */
void protocol_delete_last_profile(uint32_t arg, protocol_response_t *response, uint32_t unused, uint32_t profile_id)
{
    uint8_t index = (uint8_t)arg;
    uint32_t ns = profile_id >> 8;

    if (ns >= PROFILE_NS_COUNT || !protocol_profile_slot_is_valid(index, (uint8_t)ns)) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }
    if ((ns == PROFILE_NS_BUILTIN && index == 0u) || index != g_profile_counts[ns] - 1) {
        protocol_respond_error_byte(response, PROTOCOL_EINVAL);
        return;
    }

    int result = storage_delete_profile_dir(PROFILE_ID_RAW(index, ns));

    if (result != STORAGE_OK) {
        protocol_respond_error_byte(response, (uint8_t)-result);
        return;
    }
    response->status = PROTOCOL_STATUS_OK;

    if (ns != PROFILE_NS_BUILTIN) {
        /* Forget the host selection if it named the deleted profile. */
        if (g_profile_lp_selected != 0u && g_profile_lp_selected_id.index == index
            && g_profile_lp_selected_id.ns == PROFILE_NS_LP) {
            g_profile_lp_selected = 0u;
            g_profile_lp_selected_id.raw = 0u;
        }
        if (g_profile_active_id.index == index && g_profile_active_id.ns == PROFILE_NS_LP) {
            protocol_request_profile_switch(g_profile_lp_return_id);
        }
        g_profile_counts[PROFILE_COUNTS_LP] = index;
    } else {
        /* Everything that points at or behind the deleted profile moves to the new last one. */
        if (g_profile_active_id.ns == PROFILE_NS_BUILTIN && g_profile_active_id.index >= index) {
            g_profile_switch_pending = 1u;
            g_profile_switch_target.index = (uint8_t)(index - 1u);
            g_profile_switch_target.ns = PROFILE_NS_BUILTIN;
        }
        if (g_profile_toggle_target_id.index >= index) {
            /* The toggle target is never profile 0. */
            g_profile_toggle_target_id.index = (uint8_t)(((index < 2u) ? 2u : index) - 1u);
            g_profile_toggle_target_id.ns = PROFILE_NS_BUILTIN;
        }
        if (g_profile_lp_return_id.ns == PROFILE_NS_BUILTIN && g_profile_lp_return_id.index >= index) {
            g_profile_lp_return_id.index = (uint8_t)(index - 1u);
            g_profile_lp_return_id.ns = PROFILE_NS_BUILTIN;
        }
        g_profile_counts[PROFILE_COUNTS_BUILTIN] = index;
        signals_publish_if_changed(RGB_SIGNAL_PROFILE_COUNT, (float)index);
    }

    /* The event itself is sent by the main loop (protocol_send_profiles_changed_event). */
    g_protocol_notify_pending = 1u;
}

/* Sends the "profiles changed" event if the host subscribed to it. Called by the main loop
 * while g_protocol_notify_pending is set and the TX ring has room. */
void protocol_send_profiles_changed_event(void)
{
    Event event;

    memset(&event, 0, sizeof(event));
    event.which_event = Event_profiles_changed_tag;

    if (g_protocol_event_subscriptions[PROTOCOL_EVENT_PROFILES_CHANGED] != 0u) {
        protocol_send_event(&event);
    }
    g_protocol_notify_pending = 0u;
}

/* ---- General profile settings (file "cfg") ----------------------------------------------------- */

/* Feature command 42. arg bits 15:0 = profile id. Writes the live settings to the "cfg" file
 * of that profile. Reply: status OK, or 0xFF for an invalid id. */
static void protocol_cmd_save_live_cfg(uint32_t arg, protocol_response_t *response)
{
    profile_id_t id = { .raw = (uint16_t)arg };

    if (id.raw == PROFILE_ID_NONE || !protocol_profile_slot_is_valid(id.index, id.ns)) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    ProfileConfigFile file;

    profile_export_cfg(&file.config);
    if (g_storage_mounted != 0u) {
        storage_write_profile_pb_file(id.raw, "cfg", &ProfileConfigFile_msg, PROTOCOL_CFG_FILE_MAX, &file);
    }
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_SAVE_LIVE_CFG, protocol_cmd_save_live_cfg);

/* OUT-report command 17. Payload: ProfileConfigFile message. Without the persist flag the
 * settings are applied to RAM only; with it they are written to the addressed profile and
 * applied only if that is the active profile. No response. */
static void protocol_cmd_set_profile_cfg(const uint8_t *request)
{
    const protocol_report_profile_t *report = (const protocol_report_profile_t *)request;
    profile_id_t id = { .raw = protocol_report_profile_id(report->flags) };
    ProfileConfigFile file = ProfileConfigFile_init_default;
    pb_istream_t stream = pb_istream_from_buffer(report->payload, report->length);

    if (!pb_decode(&stream, &ProfileConfigFile_msg, &file)) {
        return;
    }
    profile_sanitize_cfg_distances(&file.config);

    if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) != 0u) {
        if (g_storage_mounted != 0u) {
            storage_write_profile_pb_file(id.raw, "cfg", &ProfileConfigFile_msg, PROTOCOL_CFG_FILE_MAX, &file);
        }
        if (g_profile_active_id.index != id.index || g_profile_active_id.ns != id.ns) {
            return;
        }
    }
    profile_apply_cfg(&file.config, 1u);
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_PROFILE_CFG, protocol_cmd_set_profile_cfg);

/* Feature command 39. arg bits 15:0 = profile id, 0xFFFF = the live settings. Reply: the
 * ProfileConfigFile message; empty payload if the profile has no "cfg" file. */
static void protocol_cmd_get_profile_cfg(uint32_t arg, protocol_response_t *response)
{
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);
    ProfileConfigFile file = ProfileConfigFile_init_default;
    uint16_t profile_id = (uint16_t)arg;

    if (profile_id == PROFILE_ID_NONE) {
        profile_export_cfg(&file.config);
    } else {
        char path[PROTOCOL_PROFILE_PATH_MAX];

        if (g_storage_mounted == 0u) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
        storage_format_profile_path(path, profile_id, "cfg");

        int result = storage_read_pb_file(path, &ProfileConfigFile_msg, &file);

        if (result == LFS_ERR_NOENT) {
            response->length = 0u;
            response->status = PROTOCOL_STATUS_OK;
            return;
        }
        if (result != STORAGE_OK) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
        profile_sanitize_cfg_distances(&file.config);
    }

    if (pb_encode(&stream, &ProfileConfigFile_msg, &file)) {
        response->length = (uint16_t)stream.bytes_written;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_PROFILE_CFG, protocol_cmd_get_profile_cfg);

/* ---- Profile info (file "i") ------------------------------------------------------------------- */

/* OUT-report command 26. Payload: ProfileInfo message; with the persist flag it is written
 * to the file "i" of the addressed profile. The profile colour signal is updated.
 * No response. */
static void protocol_cmd_set_profile_info(const uint8_t *request)
{
    const protocol_report_profile_t *report = (const protocol_report_profile_t *)request;
    uint16_t profile_id = protocol_report_profile_id(report->flags);
    ProfileInfo info;

    memset(&info, 0, sizeof(info));

    pb_istream_t stream = pb_istream_from_buffer(report->payload, report->length);

    if (!pb_decode(&stream, &ProfileInfo_msg, &info)) {
        return;
    }

    if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) != 0u && g_storage_mounted != 0u) {
        storage_write_profile_pb_file(profile_id, "i", &ProfileInfo_msg, PROTOCOL_INFO_FILE_MAX, &info);
    }

    /* KNOWN ISSUE: the profile colour signal is updated even when the addressed profile is
     * not the active one, and without testing info.has_color. The signal carries the raw
     * bits 0xFFRRGGBB in a float, which is a NaN: the comparison never finds it equal, so
     * the signal is written on every request. Matches stock firmware 2.14.1. */
    uint32_t color_bits = 0xFF000000u | ((uint32_t)info.color.r << 16) | ((uint32_t)info.color.g << 8) | info.color.b;
    float color_value;

    memcpy(&color_value, &color_bits, sizeof(color_value));
    signals_publish_if_changed(RGB_SIGNAL_PROFILE_COLOR, color_value);
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_PROFILE_INFO, protocol_cmd_set_profile_info);

/* Feature command 55. arg bits 15:0 = profile id. Reply: the ProfileInfo message of the
 * file "i"; empty payload if the file does not exist. */
static void protocol_cmd_get_profile_info(uint32_t arg, protocol_response_t *response)
{
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);
    uint16_t profile_id = (uint16_t)arg;
    char path[PROTOCOL_PROFILE_PATH_MAX];
    ProfileInfo info;

    memset(&info, 0, sizeof(info));

    if (profile_id == PROFILE_ID_NONE || g_storage_mounted == 0u) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }
    storage_format_profile_path(path, profile_id, "i");

    int result = storage_read_pb_file(path, &ProfileInfo_msg, &info);

    if (result == LFS_ERR_NOENT) {
        response->length = 0u;
        response->status = PROTOCOL_STATUS_OK;
    } else if (result == STORAGE_OK && pb_encode(&stream, &ProfileInfo_msg, &info)) {
        response->length = (uint16_t)stream.bytes_written;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_PROFILE_INFO, protocol_cmd_get_profile_info);

/* ---- Revision counter (file "pst") ------------------------------------------------------------- */

/* Reads the uint32 revision counter of a profile directory. Returns STORAGE_OK or the
 * littlefs error truncated to 16 bits. */
static int16_t protocol_read_profile_revision(uint16_t profile_id, uint32_t *revision)
{
    char path[PROTOCOL_PROFILE_PATH_MAX];
    uint8_t file_buffer[STORAGE_LFS_CACHE_SIZE];
    struct lfs_file_config file_cfg = { .buffer = file_buffer, .attrs = NULL, .attr_count = 0u };
    lfs_file_t file;
    uint32_t value = 0u;

    storage_format_profile_path(path, profile_id, "pst");

    /* KNOWN ISSUE: when the read fails the file is not closed and stays linked in the list
     * of open files of g_storage_lfs although it lives on this stack. Matches stock
     * firmware 2.14.1. */
    int err = lfs_file_opencfg(&g_storage_lfs, &file, path, STORAGE_OPEN_READ, &file_cfg);

    if (err >= 0) {
        err = lfs_file_read(&g_storage_lfs, &file, &value, sizeof(value));
        if (err >= 0) {
            *revision = value;
            err = lfs_file_close(&g_storage_lfs, &file);
        }
    }
    return (err < 0) ? (int16_t)err : (int16_t)STORAGE_OK;
}

/* Feature command 72. arg bits 15:0 = profile id. Reply: the uint32 revision counter of the
 * profile; empty payload if the file "pst" does not exist; on other errors status 0xFF and
 * the int16 error code. g_storage_mounted is not tested. */
static void protocol_cmd_get_profile_revision(uint32_t arg, protocol_response_t *response)
{
    profile_id_t id = { .raw = (uint16_t)arg };
    uint32_t revision = 0u;

    if (id.raw == PROFILE_ID_NONE || id.ns >= PROFILE_NS_COUNT || id.index >= g_profile_counts[id.ns]) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    int16_t result = protocol_read_profile_revision(id.raw, &revision);

    if (result == STORAGE_OK) {
        response->status = PROTOCOL_STATUS_OK;
        response->length = sizeof(revision);
        memcpy(response->payload, &revision, sizeof(revision));
    } else if (result == LFS_ERR_NOENT) {
        response->status = PROTOCOL_STATUS_OK;
        response->length = 0u;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
        response->length = sizeof(result);
        memcpy(response->payload, &result, sizeof(result));
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_PROFILE_REVISION, protocol_cmd_get_profile_revision);
