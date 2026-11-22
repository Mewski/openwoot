/*
 * protocol_keys_switch.c - host commands for the hall switches under the keys.
 *
 * Feature command 83 lists the switch types the firmware knows, feature command 82 /
 * OUT-report command 40 read and write the switch profiles (root file "sw": which switch
 * type sits under which keys). A locked device (g_profile_lock_mode 1 or 3) only reports
 * the edition default and ignores writes.
 */
#include <string.h>

#include "pb_decode.h"
#include "pb_encode.h"

#include "analog/analog.h"
#include "profile/profile.h"
#include "profile/edition/edition.h"
#include "proto/settings.pb.h"
#include "protocol/protocol.h"
#include "protocol/protocol_keys_internal.h"
#include "storage/storage.h"
#include "tables/analog_tables.h"

/* Constant fields 2 and 3 of every switch-type entry; meaning unknown. */
#define PROTOCOL_SWITCH_TYPE_F2  20u
#define PROTOCOL_SWITCH_TYPE_F3  2u

/* arg byte 0 of feature command 82. */
enum protocol_switch_profiles_source {
    PROTOCOL_SWITCH_PROFILES_FILE    = 0x00,  /* root file "sw" */
    PROTOCOL_SWITCH_PROFILES_FACTORY = 0x01,  /* factory default of this edition */
    PROTOCOL_SWITCH_PROFILES_LIVE    = 0xFF,  /* g_analog_switch_profiles */
};

/* SerialNumber.edition of the keyboards that ship without a default switch type. */
#define PROTOCOL_EDITION_NO_DEFAULT_SWITCH  1u

/* Fills one entry of the switch-type list from the switch tables. */
static void protocol_fill_switch_type_entry(SwitchTypeEntry *entry, uint8_t switch_type)
{
    entry->id = (SwitchType)switch_type;
    entry->f2 = PROTOCOL_SWITCH_TYPE_F2;
    entry->f3 = PROTOCOL_SWITCH_TYPE_F3;
    entry->travel_0p05mm = k_analog_switch_travel[switch_type];
    entry->has_f5 = true;
    entry->f5 = k_analog_switch_group_id[switch_type];
}

/* Feature command 83. arg byte 0 = index of the first entry. Reply: SwitchTypeList message
 * with the switch types the firmware knows from that index on and their total number; on a
 * locked device only the default switch type of the edition. */
static void protocol_cmd_get_switch_types(uint32_t arg, protocol_response_t *response)
{
    SwitchTypeList list;

    memset(&list, 0, sizeof(list));
    list.has_total_count = true;

    if (PROFILE_LOCK_MODE_IS_LOCKED(g_profile_lock_mode)) {
        list.total_count = 1;
        list.entries_count = 1;
        protocol_fill_switch_type_entry(&list.entries[0], profile_edition_default_switch_type());
    } else {
        uint8_t first = (uint8_t)arg;

        if (first >= ANALOG_SWITCH_TYPE_COUNT) {
            first = ANALOG_SWITCH_TYPE_COUNT;
        }
        list.total_count = ANALOG_SWITCH_TYPE_COUNT;
        list.entries_count = (pb_size_t)(ANALOG_SWITCH_TYPE_COUNT - first);
        for (uint8_t i = 0; i < list.entries_count; ++i) {
            protocol_fill_switch_type_entry(&list.entries[i], (uint8_t)(i + first));
        }
    }

    protocol_reply_message(response, &SwitchTypeList_msg, &list);
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_SWITCH_TYPES, protocol_cmd_get_switch_types);

/* Factory default of the switch profiles: no entries, optional default switch type, file
 * version 1. `profiles` must be zeroed. */
static void protocol_fill_factory_switch_profiles(SwitchProfiles *profiles, bool with_switch_type)
{
    profiles->has_default_params = true;
    profiles->entries_count = 0;
    if (with_switch_type) {
        profiles->default_params.which_type = SwitchParams_switch_type_tag;
        profiles->default_params.type.switch_type = (SwitchType)profile_edition_default_switch_type();
        profiles->default_params.has_travel_offset = false;
    } else {
        profiles->default_params.which_type = 0;
    }
    profiles->has_version = true;
    profiles->version = 1;
}

/* Feature command 82. arg byte 0 = source: 0x00 root file "sw", 0x01 factory default of this
 * edition, 0xFF the live state. Reply: SwitchProfiles message; empty payload if the file
 * does not exist; status 0xFF for another source. A locked device always answers with the
 * edition default. */
static void protocol_cmd_get_switch_profiles(uint32_t arg, protocol_response_t *response)
{
    SwitchProfiles profiles;

    if (PROFILE_LOCK_MODE_IS_LOCKED(g_profile_lock_mode)) {
        /* Locked: the request is ignored, the edition default is the only answer. */
        memset(&profiles, 0, sizeof(profiles));
        protocol_fill_factory_switch_profiles(&profiles, true);
        protocol_reply_message(response, &SwitchProfiles_msg, &profiles);
        return;
    }

    switch ((uint8_t)arg) {
    case PROTOCOL_SWITCH_PROFILES_LIVE:
        protocol_reply_message(response, &SwitchProfiles_msg, &g_analog_switch_profiles);
        break;

    case PROTOCOL_SWITCH_PROFILES_FILE: {
        int result;

        memset(&profiles, 0, sizeof(profiles));
        result = analog_load_switch_profiles(&profiles);
        if (result == 1) {
            protocol_reply_message(response, &SwitchProfiles_msg, &profiles);
        } else if (result == LFS_ERR_NOENT) {
            protocol_reply_empty(response);
        } else {
            response->status = PROTOCOL_STATUS_ERROR;
        }
        break;
    }

    case PROTOCOL_SWITCH_PROFILES_FACTORY: {
        DeviceInfo info;
        bool no_default_switch;

        memset(&profiles, 0, sizeof(profiles));
        memset(&info, 0, sizeof(info));
        no_default_switch = storage_read_device_info(&info) == 1
                         && info.serial_number.has_edition
                         && info.serial_number.edition == PROTOCOL_EDITION_NO_DEFAULT_SWITCH;
        protocol_fill_factory_switch_profiles(&profiles, !no_default_switch);
        protocol_reply_message(response, &SwitchProfiles_msg, &profiles);
        break;
    }

    default:
        response->status = PROTOCOL_STATUS_ERROR;
        break;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_SWITCH_PROFILES, protocol_cmd_get_switch_profiles);

/* Body of OUT-report command 40. Payload: SwitchProfiles message. Missing switch types are
 * filled in, the result becomes the live state and, with the persist flag, is written to
 * the root file "sw". No response. */
static void protocol_cmd_set_switch_profiles_body(const uint8_t *request)
{
    const protocol_report_profile_t *report = (const protocol_report_profile_t *)request;
    pb_istream_t in_stream = pb_istream_from_buffer(report->payload, report->length);
    SwitchProfiles profiles;

    if (!pb_decode(&in_stream, &SwitchProfiles_msg, &profiles)) {
        return;
    }

    profiles.has_version = true;
    profiles.version = 1;
    analog_fill_missing_switch_types(&profiles);

    if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
        uint8_t encoded[SwitchProfiles_size];
        pb_ostream_t out_stream = pb_ostream_from_buffer(encoded, sizeof(encoded));

        pb_encode(&out_stream, &SwitchProfiles_msg, &profiles);
        storage_write_file("sw", encoded, (uint16_t)out_stream.bytes_written);
    }

    g_analog_switch_profiles = profiles;
    analog_apply_switch_profiles();
}

/* OUT-report command 40. Sets the switch profiles (see
 * protocol_cmd_set_switch_profiles_body); ignored on a locked device. No response. */
static void protocol_cmd_set_switch_profiles(const uint8_t *request)
{
    if (!PROFILE_LOCK_MODE_IS_LOCKED(g_profile_lock_mode)) {
        protocol_cmd_set_switch_profiles_body(request);
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_SWITCH_PROFILES, protocol_cmd_set_switch_profiles);
