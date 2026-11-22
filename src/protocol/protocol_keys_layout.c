/*
 * protocol_keys_layout.c - host commands for keymap layers and "al<k>" files.
 *
 * Keymap layers (profile files "ml0".."ml3", raw 6 x 21 key codes): feature command 48 reads
 * one, OUT-report command 20 sets, stores or resets one.
 * "al<k>" files (k = 0..15, only in "lp/<n>/" profiles): opaque AlFile messages the firmware
 * merely stores for the host; feature commands 86 (count) and 87 (read), OUT-report
 * command 44 (write / delete).
 */
#include <stdio.h>
#include <string.h>

#include "pb_decode.h"
#include "pb_encode.h"

#include "layout/layout.h"
#include "profile/profile.h"
#include "proto/settings.pb.h"
#include "protocol/protocol.h"
#include "protocol/protocol_keys_internal.h"
#include "storage/storage.h"

#define PROTOCOL_FILE_NAME_SIZE    16u  /* "ml<k>" / "al<k>" */
#define PROTOCOL_DIR_NAME_SIZE     8u   /* "<index>" / "lp/<index>" */
#define PROTOCOL_FILE_PATH_SIZE    28u  /* "<dir>/<name>" */

#define PROTOCOL_AL_FILE_MAX       15u  /* highest al slot */

/* Highest profile index per namespace accepted by feature command 48. */
#define PROTOCOL_BUILTIN_PROFILE_INDEX_MAX  3u
#define PROTOCOL_LP_PROFILE_INDEX_MAX       9u

/* Result convention of the storage helpers used by OUT-report command 44: 1 = ok, else the
 * littlefs error truncated to 16 bits. */
#define PROTOCOL_STORAGE_OK        1

/* ---- Keymap layers ----------------------------------------------------------------------------- */

/* Builds the path of a keymap layer file: "<index>/ml<layer>" or "lp/<index>/ml<layer>"; an
 * unknown namespace gives "/ml<layer>". */
static void protocol_format_keymap_layer_path(char *path, uint16_t profile_id, uint32_t layer)
{
    char name[PROTOCOL_FILE_NAME_SIZE];
    char dir[PROTOCOL_DIR_NAME_SIZE];

    sprintf(name, "ml%d", (int)layer);
    if (PROFILE_ID_NS(profile_id) == PROFILE_NS_BUILTIN) {
        sprintf(dir, "%d", PROFILE_ID_INDEX(profile_id));
    } else if (PROFILE_ID_NS(profile_id) == PROFILE_NS_LP) {
        sprintf(dir, "lp/%d", PROFILE_ID_INDEX(profile_id));
    } else {
        dir[0] = '\0';
    }
    sprintf(path, "%s/%s", dir, name);
}

/* Feature command 48. arg bits 15:0 = profile id (0xFFFF = the active profile), arg byte 2 =
 * layer. Reply: the KeymapLayer message of the file "ml<layer>"; empty payload if the file
 * does not exist; status 0xFF for an invalid profile id. */
static void protocol_cmd_get_keymap_layer(uint32_t arg, protocol_response_t *response)
{
    uint8_t layer = (uint8_t)(arg >> 16);
    uint8_t index;
    uint8_t ns;
    KeymapLayer keymap;
    int result;

    if ((uint16_t)arg == PROFILE_ID_NONE) {
        ns = g_profile_active_id.ns;
        index = g_profile_active_id.index;
    } else {
        ns = (uint8_t)(arg >> 8);
        index = (uint8_t)arg;
    }

    if (ns == PROFILE_NS_BUILTIN) {
        if (index > PROTOCOL_BUILTIN_PROFILE_INDEX_MAX) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
    } else if (ns != PROFILE_NS_LP || index > PROTOCOL_LP_PROFILE_INDEX_MAX) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    memset(&keymap, 0, sizeof(keymap));
    result = storage_read_keymap_layer(PROFILE_ID_RAW(index, ns), &keymap, layer);
    if (result == 1) {
        protocol_reply_message(response, &KeymapLayer_msg, &keymap);
    } else if (result == LFS_ERR_NOENT) {
        protocol_reply_empty(response);
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_KEYMAP_LAYER, protocol_cmd_get_keymap_layer);

/* Highest keymap layer of a profile for the top-layer bits of g_layout_layer_state, from the
 * number of consecutive "ml<k>" files. Without any stored layer the board's default layers
 * are in use. */
static uint8_t protocol_find_top_keymap_layer(uint16_t profile_id)
{
    char path[PROTOCOL_FILE_PATH_SIZE];
    uint8_t layer;

    if (g_storage_mounted == 0) {
        return BOARD_DEFAULT_KEYMAP_LAYERS - 1;
    }
    for (layer = 0; layer != LAYOUT_LAYERS; ++layer) {
        protocol_format_keymap_layer_path(path, profile_id, layer);
        if (storage_file_exists(path) == 0) {
            break;
        }
    }
    if (layer == LAYOUT_LAYERS) {
        return LAYOUT_LAYERS - 1;
    }
    /* `layer` is the first missing file. */
    return (layer != 0) ? (uint8_t)((layer - 1) & LAYOUT_STATE_TOP_LAYER_MASK)
                        : (uint8_t)(BOARD_DEFAULT_KEYMAP_LAYERS - 1);
}

/* Puts the stored (or factory) keymap of one layer of the active profile back into the key
 * records. The persist bit is read from the report buffer after the keymap was applied. */
static void protocol_reset_keymap_layer(uint32_t layer, const protocol_report_indexed_t *request)
{
    uint8_t keymap[LAYOUT_KEYMAP_SIZE];
    uint16_t source_id;

    if (layer == 0) {
        layout_load_keymap_layers(g_profile_active_id.raw);
        return;
    }

    /* Layers come from the active profile only if its layer 0 does; else from profile 0. */
    if (g_layout_layer_state & LAYOUT_STATE_OWN_KEYMAP) {
        source_id = PROFILE_ID_RAW(g_profile_active_id.index, g_profile_active_id.ns);
    } else {
        source_id = PROFILE_ID_RAW(0, PROFILE_NS_BUILTIN);
    }

    if (storage_read_keymap_layer(source_id, keymap, layer) != 1) {
        layout_get_default_keymap(layer, keymap);
    }
    layout_apply_keymap_layer(keymap, layer);

    if (request->flags & PROTOCOL_REPORT_FLAG_PERSIST) {
        uint8_t top_layer = protocol_find_top_keymap_layer(source_id);

        g_layout_layer_state = (uint8_t)((g_layout_layer_state & ~LAYOUT_STATE_TOP_LAYER_MASK)
                                         | (top_layer & LAYOUT_STATE_TOP_LAYER_MASK));
    }
}

/* OUT-report command 20. index = layer 0..3, payload: KeymapLayer message; an empty payload
 * resets the layer to the stored or factory keymap. With the persist flag the file
 * "ml<layer>" of the addressed profile is written (or removed for an empty payload). No
 * response. */
static void protocol_cmd_set_keymap_layer(const uint8_t *report)
{
    const protocol_report_indexed_t *request = (const protocol_report_indexed_t *)report;
    uint32_t layer = request->index;
    uint16_t length;
    uint16_t profile_id;
    KeymapLayer keymap;

    if (layer >= LAYOUT_LAYERS) {
        return;
    }
    length = request->length;
    profile_id = protocol_report_profile_id(request->flags);
    memset(&keymap, 0, sizeof(keymap));

    if (length != 0) {
        pb_istream_t stream = pb_istream_from_buffer(request->payload, length);

        if (!pb_decode(&stream, &KeymapLayer_msg, &keymap)) {
            return;
        }
        if (request->flags & PROTOCOL_REPORT_FLAG_PERSIST) {
            storage_write_keymap_layer(profile_id, &keymap, layer);
        }
    } else if ((request->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
        char path[PROTOCOL_FILE_PATH_SIZE];

        protocol_format_keymap_layer_path(path, profile_id, layer);
        storage_bump_profile_revision(profile_id);
        lfs_remove(&g_storage_lfs, path);
    }

    /* Without the persist bit the request is a live preview; a stored change reaches the RAM
     * state only when it addresses the active profile. */
    if ((request->flags & PROTOCOL_REPORT_FLAG_PERSIST) && !protocol_profile_is_active(profile_id)) {
        return;
    }

    if (length == 0) {
        protocol_reset_keymap_layer(layer, request);
        return;
    }

    layout_apply_keymap_layer((const void *)&keymap, layer);

    uint8_t top_layer = g_layout_layer_state & LAYOUT_STATE_TOP_LAYER_MASK;
    if (top_layer < layer) {
        top_layer = (uint8_t)layer;
    }
    g_layout_layer_state = (uint8_t)((g_layout_layer_state & ~LAYOUT_STATE_TOP_LAYER_MASK)
                                     | (top_layer & LAYOUT_STATE_TOP_LAYER_MASK));
    if (layer == 0 && (request->flags & PROTOCOL_REPORT_FLAG_PERSIST)) {
        g_layout_layer_state |= LAYOUT_STATE_OWN_KEYMAP;
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_KEYMAP_LAYER, protocol_cmd_set_keymap_layer);

/* ---- "al<k>" files ----------------------------------------------------------------------------- */

/* Builds the path "lp/<profile_index>/al<slot>". */
static void protocol_format_al_path(char *path, uint8_t profile_index, uint8_t slot)
{
    char name[PROTOCOL_FILE_NAME_SIZE];
    char dir[PROTOCOL_DIR_NAME_SIZE];

    sprintf(name, "al%d", slot);
    sprintf(dir, "lp/%d", profile_index);
    sprintf(path, "%s/%s", dir, name);
}

/* Sends the same event as protocol_send_profiles_changed_event, but g_protocol_notify_pending
 * is left alone. */
static void protocol_notify_al_files_changed(void)
{
    Event event;

    memset(&event, 0, sizeof(event));
    event.which_event = Event_profiles_changed_tag;
    if (g_protocol_event_subscriptions[PROTOCOL_EVENT_PROFILES_CHANGED] != 0) {
        protocol_send_event(&event);
    }
}

/* Feature command 86. arg bits 15:0 = profile id in the "lp" namespace. Reply: one byte, the
 * number of "al<k>" files of that profile; status 0xFF for any other id or a storage error. */
static void protocol_cmd_get_al_file_count(uint32_t arg, protocol_response_t *response)
{
    uint16_t profile_id = (uint16_t)arg;
    uint8_t count;

    if (profile_id != PROFILE_ID_NONE
        && PROFILE_ID_NS(profile_id) == PROFILE_NS_LP
        && storage_count_al_files(profile_id, &count) == 1) {
        response->length = 1;
        response->status = PROTOCOL_STATUS_OK;
        response->payload[0] = count;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_AL_FILE_COUNT, protocol_cmd_get_al_file_count);

/* OUT-report command 44. index = slot 0..15, payload: AlFile message. Only handled for
 * profiles of the "lp" namespace and with the persist flag. The message is written to the
 * file "al<slot>" (an existing slot or the next free one); an empty payload deletes the file
 * if it is the last one. Sends the "profiles changed" event afterwards. No response. */
static void protocol_cmd_set_al_file(const uint8_t *request)
{
    const protocol_report_indexed_t *report = (const protocol_report_indexed_t *)request;
    uint8_t flags = report->flags;
    uint8_t slot = report->index;
    uint16_t length;
    uint16_t profile_id;
    uint8_t count = 0;
    char path[PROTOCOL_FILE_PATH_SIZE];
    int result;

    if ((flags >> PROTOCOL_REPORT_FLAG_NS_SHIFT) != PROFILE_NS_LP) {
        return;
    }
    if (slot > PROTOCOL_AL_FILE_MAX || !(flags & PROTOCOL_REPORT_FLAG_PERSIST)) {
        return;
    }
    length = report->length;
    profile_id = protocol_report_profile_id(flags);

    if (length == 0) {
        /* Delete: only the last file, so the slots stay consecutive. */
        if (g_storage_mounted == 0) {
            return;
        }
        if (storage_count_al_files(profile_id, &count) != 1 || count == 0 || slot != count - 1) {
            return;
        }
        protocol_format_al_path(path, PROFILE_ID_INDEX(profile_id), slot);
        storage_bump_profile_revision(profile_id);
        result = lfs_remove(&g_storage_lfs, path);
        if (result < 0 && (int16_t)result != PROTOCOL_STORAGE_OK) {
            return;
        }
    } else {
        /* Write: replace an existing file or append the next one. */
        AlFile al_file;
        uint8_t encoded[AlFile_size];
        pb_istream_t in_stream = pb_istream_from_buffer(report->payload, length);
        pb_ostream_t out_stream;

        memset(&al_file, 0, sizeof(al_file));
        if (!pb_decode(&in_stream, &AlFile_msg, &al_file)) {
            return;
        }
        if (g_storage_mounted == 0) {
            return;
        }
        if (storage_count_al_files(profile_id, &count) != 1 || count < slot) {
            return;
        }
        protocol_format_al_path(path, PROFILE_ID_INDEX(profile_id), slot);
        storage_bump_profile_revision(profile_id);

        out_stream = pb_ostream_from_buffer(encoded, sizeof(encoded));
        pb_encode(&out_stream, &AlFile_msg, &al_file);
        result = storage_write_file(path, encoded, (uint16_t)out_stream.bytes_written);
        if (result <= 0 && (int16_t)result != PROTOCOL_STORAGE_OK) {
            return;
        }
    }

    protocol_notify_al_files_changed();
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_AL_FILE, protocol_cmd_set_al_file);

/* Feature command 87. arg bits 15:0 = profile id in the "lp" namespace, arg byte 2 = slot
 * 0..15. Reply: the AlFile message of the file "al<slot>"; empty payload if the file does
 * not exist; status 0xAA for id 0xFFFF, 0xFF for any other invalid request. */
static void protocol_cmd_get_profile_al_file(uint32_t arg, protocol_response_t *response)
{
    uint16_t profile_id = (uint16_t)arg;
    uint8_t slot = (uint8_t)(arg >> 16);
    char path[PROTOCOL_FILE_PATH_SIZE];
    AlFile al_file;
    int result;

    if (profile_id == PROFILE_ID_NONE) {
        response->status = PROTOCOL_STATUS_UNSUPPORTED;
        return;
    }
    if (PROFILE_ID_NS(profile_id) != PROFILE_NS_LP || slot > PROTOCOL_AL_FILE_MAX) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    memset(&al_file, 0, sizeof(al_file));
    if (g_storage_mounted == 0) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    protocol_format_al_path(path, PROFILE_ID_INDEX(profile_id), slot);
    if (storage_file_exists(path) == 0) {
        protocol_reply_empty(response);
        return;
    }

    result = storage_read_pb_file(path, &AlFile_msg, &al_file);
    if (result == 1) {
        protocol_reply_message(response, &AlFile_msg, &al_file);
    } else if (result == LFS_ERR_NOENT) {
        protocol_reply_empty(response);
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_PROFILE_AL_FILE, protocol_cmd_get_profile_al_file);
