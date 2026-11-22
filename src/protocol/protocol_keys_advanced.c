/*
 * protocol_keys_advanced.c - host commands for the advanced-key list (dynamic keystroke,
 * mod-tap, toggle, two-key bindings; profile file "ak2").
 *
 * The list of up to 40 entries travels in pages of 10 (AdvancedKeysPage): feature command 52
 * reads a page, OUT-report command 24 writes one, feature command 53 stores the RAM list.
 */
#include <string.h>

#include "pb_decode.h"
#include "pb_encode.h"

#include "analog/analog.h"
#include "keys/keys.h"
#include "profile/profile.h"
#include "proto/settings.pb.h"
#include "protocol/protocol.h"
#include "protocol/protocol_keys_internal.h"
#include "storage/storage.h"

/* Highest profile index per namespace accepted by feature command 53. */
#define PROTOCOL_BUILTIN_PROFILE_INDEX_MAX  3u
#define PROTOCOL_LP_PROFILE_INDEX_MAX       9u

/* Feature command 53. arg bits 15:0 = profile id. Stores the advanced-key list held in RAM
 * in the file "ak2" of that profile. Reply: status OK, or 0xFF for an invalid id. */
static void protocol_cmd_save_advanced_keys(uint32_t arg, protocol_response_t *response)
{
    uint16_t profile_id = (uint16_t)arg;
    uint8_t index = PROFILE_ID_INDEX(profile_id);
    uint8_t ns = PROFILE_ID_NS(profile_id);
    bool valid;

    if (profile_id == PROFILE_ID_NONE) {
        valid = false;
    } else if (ns == PROFILE_NS_BUILTIN) {
        valid = index <= PROTOCOL_BUILTIN_PROFILE_INDEX_MAX;
    } else {
        valid = ns == PROFILE_NS_LP && index <= PROTOCOL_LP_PROFILE_INDEX_MAX;
    }
    if (!valid) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    /* storage_save_advanced_keys: if mounted, remove the legacy file "dks", bump the
     * revision, write "ak2". */
    storage_save_advanced_keys(profile_id, &g_keys_advanced_keys);
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_SAVE_ADVANCED_KEYS, protocol_cmd_save_advanced_keys);

/* Feature command 52. arg bits 15:0 = profile id (0xFFFF = the list in RAM), arg byte 2 =
 * index of the first entry. Reply: AdvancedKeysPage message with up to 10 entries from that
 * index and the total number of entries; empty payload if the profile has no "ak2" file. */
static void protocol_cmd_get_advanced_keys(uint32_t arg, protocol_response_t *response)
{
    uint16_t profile_id = (uint16_t)arg;
    uint8_t first = (uint8_t)(arg >> 16);
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);
    AdvancedKeys list;
    AdvancedKeysPage page;

    memset(&list, 0, sizeof(list));

    if (profile_id == PROFILE_ID_NONE) {
        memcpy(&list, &g_keys_advanced_keys, sizeof(list));
    } else {
        char path[PROTOCOL_PROFILE_PATH_SIZE];
        int result;

        if (g_storage_mounted == 0) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
        storage_format_profile_path(path, profile_id, "ak2");
        result = storage_read_pb_file(path, &AdvancedKeys_msg, &list);
        if (result != 1) {
            if (result == LFS_ERR_NOENT) {
                protocol_reply_empty(response);
            } else {
                /* KNOWN ISSUE: the low byte of the (negative) error code is sent as status
                 * instead of PROTOCOL_STATUS_ERROR. Matches stock firmware 2.14.1. */
                response->status = (uint8_t)result;
            }
            return;
        }
    }

    keys_validate_advanced_keys(&list);

    /* Everything but total_count is cleared (163 bytes). */
    memset(&page, 0, offsetof(AdvancedKeysPage, total_count));
    page.total_count = (uint8_t)list.entries_count;

    if (first < KEYS_ADVANCED_KEY_MAX) {
        uint8_t total = (uint8_t)list.entries_count;

        if (total > first) {
            uint32_t count = (uint32_t)total - first;

            if (count >= KEYS_ADVANCED_KEY_PAGE_MAX) {
                count = KEYS_ADVANCED_KEY_PAGE_MAX;
            }
            page.entries_count = (pb_size_t)count;
            memcpy(page.entries, &list.entries[first], count * sizeof(AdvancedKey));
        }
        page.first_index = first;
    }

    if (pb_encode(&stream, &AdvancedKeysPage_msg, &page)) {
        response->length = (uint16_t)stream.bytes_written;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_ADVANCED_KEYS, protocol_cmd_get_advanced_keys);

/* OUT-report command 24. Payload: AdvancedKeysPage message; pages must arrive in order. The
 * list in RAM is cut behind the page, the page entries are stored and every key is bound
 * again. An empty payload clears the list. With the persist flag the list is written to the
 * file "ak2" of the active profile. No response. */
static void protocol_cmd_set_advanced_keys(const uint8_t *request)
{
    const protocol_report_profile_t *report = (const protocol_report_profile_t *)request;
    AdvancedKeysPage page;
    uint8_t first = 0;

    keys_stop_toggle_overlays();

    /* An empty payload leaves the page zeroed: first index 0, no entries, so the list is cleared. */
    memset(&page, 0, sizeof(page));
    if (report->length != 0) {
        pb_istream_t stream = pb_istream_from_buffer(report->payload, report->length);

        if (!pb_decode(&stream, &AdvancedKeysPage_msg, &page)) {
            return;
        }
        first = page.first_index;
        if (g_keys_advanced_keys.entries_count < first) {
            return;  /* pages must arrive in order, without a gap */
        }
    }

    /* The list now ends behind this page. */
    uint32_t new_count = (uint32_t)first + page.entries_count;
    if (new_count >= KEYS_ADVANCED_KEY_MAX) {
        new_count = KEYS_ADVANCED_KEY_MAX;
    }
    g_keys_advanced_keys.entries_count = (pb_size_t)new_count;

    uint8_t index = first;
    for (uint8_t i = 0; i < page.entries_count; ++i, ++index) {
        if (index >= KEYS_ADVANCED_KEY_MAX) {
            continue;
        }
        if (index >= g_keys_advanced_keys.entries_count) {
            /* Append check; cannot trigger after the count update above. */
            if (index != g_keys_advanced_keys.entries_count) {
                continue;
            }
            g_keys_advanced_keys.entries_count = (pb_size_t)(index + 1);
        }
        memset(&g_keys_advanced_key_states[index], 0, sizeof(g_keys_advanced_key_states[index]));
        g_keys_advanced_keys.entries[index] = page.entries[i];
    }

    keys_validate_advanced_keys(&g_keys_advanced_keys);

    /* Unbind every key on all four layers, then bind the whole list again. */
    for (uint32_t pos = 0; pos != ANALOG_KEY_COUNT; ++pos) {
        memset(g_analog_key_states[pos].ak_slot, ANALOG_NO_ADVANCED_KEY, sizeof(g_analog_key_states[pos].ak_slot));
    }
    for (uint8_t i = 0; i < g_keys_advanced_keys.entries_count; ++i) {
        keys_apply_advanced_key_entry(i, &g_keys_advanced_keys.entries[i]);
    }

    if (report->flags & PROTOCOL_REPORT_FLAG_PERSIST) {
        storage_save_advanced_keys(g_profile_active_id.raw, &g_keys_advanced_keys);
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_ADVANCED_KEYS, protocol_cmd_set_advanced_keys);
