/*
 * protocol_keys_actuation.c - host commands for key scanning and per-key actuation points.
 *
 * Feature commands 20 (key sensor values), 21 / 22 (key reports off / on), 44 (raw scanning),
 * 45 / 46 (gamepad slot detection pattern), 49 (get actuation points) and OUT-report
 * command 21 (set actuation points, profile file "act").
 */
#include <string.h>

#include "pb_decode.h"
#include "pb_encode.h"

#include "adc_scan/reference/reference.h"
#include "analog/analog.h"
#include "core/run_state.h"
#include "hid/hid.h"
#include "keys/keys.h"
#include "layout/layout.h"
#include "profile/profile.h"
#include "proto/settings.pb.h"
#include "protocol/protocol.h"
#include "protocol/protocol_keys_internal.h"
#include "storage/storage.h"
#include "tables/protocol_tables.h"
#include "usb/usb.h"

/* arg byte 0 of feature command 20: which per-key value is reported. Values above 5 read as 0. */
enum protocol_sensor_kind {
    PROTOCOL_SENSOR_VALUE         = 0,  /* current value: raw sensor word or travel scaled to 0..255 */
    PROTOCOL_SENSOR_REST_SINGLE   = 1,  /* rest level of single-input scan mode */
    PROTOCOL_SENSOR_STATE_FLAG    = 2,  /* 0xFF / 0x00 from ANALOG_KEY_FLAG_SENSOR_STATE */
    PROTOCOL_SENSOR_SCAN_ORDER    = 3,  /* matrix format only: position in k_protocol_sensor_scan_order */
    PROTOCOL_SENSOR_REST_PARALLEL = 4,  /* rest level of six-input scan mode */
    PROTOCOL_SENSOR_BOTTOM        = 5,  /* bottom-out level */
    PROTOCOL_SENSOR_KIND_MAX      = 5,
};

/* arg byte 1 of feature command 20. */
#define PROTOCOL_SENSOR_FORMAT_RAW   1u   /* 96 uint16 by scan position; everything else: 6 x 21 byte matrix */

#define PROTOCOL_SCAN_ORDER_MUXES    ((unsigned)BOARD_MUX_COUNT)          /* rows of k_protocol_sensor_scan_order */
#define PROTOCOL_SCAN_ORDER_STEPS    ((unsigned)BOARD_MUX_ADDRESSES_USED) /* columns of k_protocol_sensor_scan_order */
#define PROTOCOL_SCAN_ORDER_CH_MAX   (PROTOCOL_SCAN_ORDER_STEPS - 1u)     /* table values above the last address in use are skipped */

/* Feature command 21. Stops the keyboard from sending key reports. Reply: status OK. */
static void protocol_cmd_keys_off(uint32_t arg, protocol_response_t *response)
{
    g_usb_link_flags &= (uint8_t)~USB_LINK_KEY_REPORTS;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_KEYS_OFF, protocol_cmd_keys_off);

/* Feature command 22. Enables key reports again. Reply: status OK. */
static void protocol_cmd_keys_on(uint32_t arg, protocol_response_t *response)
{
    g_usb_link_flags |= USB_LINK_KEY_REPORTS;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_KEYS_ON, protocol_cmd_keys_on);

/* Feature command 44. arg != 0 switches raw scanning on (command 20 then reports raw sensor
 * words instead of travel), arg == 0 switches it off. Reply: status OK. */
static void protocol_cmd_set_raw_scanning(uint32_t arg, protocol_response_t *response)
{
    g_run_state_options = (uint8_t)((g_run_state_options & ~RUN_STATE_OPTION_RAW_SCANNING) | (arg != 0));
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_SET_RAW_SCANNING, protocol_cmd_set_raw_scanning);

/* Feature command 45. Starts the gamepad slot detection pattern. Reply: status OK. */
static void protocol_cmd_start_xinput_detection(uint32_t arg, protocol_response_t *response)
{
    g_hid_gamepad_detection_active = 1;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_START_XINPUT_DETECTION, protocol_cmd_start_xinput_detection);

/* Feature command 46. Stops the gamepad slot detection pattern. Reply: status OK. */
static void protocol_cmd_stop_xinput_detection(uint32_t arg, protocol_response_t *response)
{
    g_hid_gamepad_detection_active = 0;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_STOP_XINPUT_DETECTION, protocol_cmd_stop_xinput_detection);

/* Travel 0..4095 of a key scaled to 0..255. */
static uint8_t protocol_key_travel_u8(const analog_key_state_t *key)
{
    return (uint8_t)((255u * analog_compute_key_travel(key)) / ANALOG_TRAVEL_MAX);
}

/* Feature command 20, format 1: one uint16 per scan position. */
static void protocol_report_sensor_values_raw(uint8_t kind, protocol_response_t *response)
{
    uint8_t *out = response->payload;

    for (uint32_t pos = 0; pos != ANALOG_KEY_COUNT; ++pos) {
        const analog_key_state_t *key = &g_analog_key_states[pos];
        uint16_t value;

        switch (kind) {
        case PROTOCOL_SENSOR_REST_SINGLE:
            value = key->rest_level_single;
            break;
        case PROTOCOL_SENSOR_STATE_FLAG:
            value = (key->flags & ANALOG_KEY_FLAG_SENSOR_STATE) ? 0xFFu : 0u;
            break;
        case PROTOCOL_SENSOR_REST_PARALLEL:
            value = key->rest_level_parallel;
            break;
        case PROTOCOL_SENSOR_BOTTOM:
            value = key->bottom_level;
            break;
        default:
            if (g_run_state_options & RUN_STATE_OPTION_RAW_SCANNING) {
                value = key->value;
            } else {
                value = protocol_key_travel_u8(key);
            }
            break;
        }
        memcpy(out, &value, sizeof(value));
        out += sizeof(value);
    }

    response->length = ANALOG_KEY_COUNT * sizeof(uint16_t);
    response->status = PROTOCOL_STATUS_OK;
}

/* Feature command 20, kind 3: the cell of every key listed in k_protocol_sensor_scan_order
 * gets mux + 10 * step. The table is all zero in this firmware. */
static void protocol_fill_scan_order_matrix(KeyMatrixU8 *matrix)
{
    uint8_t *cells = (uint8_t *)matrix;

    for (uint32_t mux = 0; mux != PROTOCOL_SCAN_ORDER_MUXES; ++mux) {
        for (uint32_t step = 0; step != PROTOCOL_SCAN_ORDER_STEPS; ++step) {
            uint8_t channel = k_protocol_sensor_scan_order[mux][step];

            if (channel > PROTOCOL_SCAN_ORDER_CH_MAX) {
                continue;
            }
            uint8_t key_id = g_layout_scanpos_to_keyid[(uint8_t)(channel + BOARD_MUX_POSITIONS * mux)];
            if (key_id != LAYOUT_NO_KEY) {
                /* Not range checked: row * LAYOUT_KEYMAP_COLS + col straight into the matrix. */
                cells[LAYOUT_KEYMAP_COLS * KEYS_KEY_ID_ROW(key_id) + KEYS_KEY_ID_COL(key_id)] =
                    (uint8_t)(mux + 10u * step);
            }
        }
    }
}

/* Feature command 20, matrix format: one byte per key id. */
static void protocol_fill_sensor_matrix(KeyMatrixU8 *matrix, uint8_t kind)
{
    for (uint32_t row = 0; row != LAYOUT_ROWS; ++row) {
        for (uint32_t col = 0; col != LAYOUT_COLS; ++col) {
            uint8_t *cell = &matrix->rows[row].values[col];
            bool raw_scanning = (g_run_state_options & RUN_STATE_OPTION_RAW_SCANNING) != 0;

            if (raw_scanning && kind == PROTOCOL_SENSOR_VALUE && row == 5 && col == 5) {
                *cell = adc_scan_reference_report();  /* this key's cell carries the zero-field reference */
                continue;
            }

            uint8_t pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];
            if (pos == LAYOUT_NO_KEY) {
                continue;
            }
            const analog_key_state_t *key = &g_analog_key_states[pos];

            switch (kind) {
            case PROTOCOL_SENSOR_REST_SINGLE:
                *cell = (uint8_t)(key->rest_level_single >> 4);
                break;
            case PROTOCOL_SENSOR_STATE_FLAG:
                *cell = (key->flags & ANALOG_KEY_FLAG_SENSOR_STATE) ? 0xFFu : 0u;
                break;
            case PROTOCOL_SENSOR_REST_PARALLEL:
                *cell = (uint8_t)(key->rest_level_parallel >> 4);
                break;
            case PROTOCOL_SENSOR_BOTTOM:
                *cell = (uint8_t)(key->bottom_level >> 4);
                break;
            default:
                if (raw_scanning) {
                    *cell = (uint8_t)(key->value >> 4);
                } else {
                    *cell = protocol_key_travel_u8(key);
                }
                break;
            }
        }
    }
}

/* Feature command 20. arg byte 0 = kind of value (enum protocol_sensor_kind), arg byte 1 =
 * format. Reply: format 1: 96 uint16 by scan position; otherwise a KeyMatrixU8 message with
 * one byte per key. */
static void protocol_cmd_get_key_sensor_values(uint32_t arg, protocol_response_t *response)
{
    uint8_t kind = (uint8_t)arg;
    uint8_t format = (uint8_t)(arg >> 8);

    if (kind > PROTOCOL_SENSOR_KIND_MAX) {
        kind = PROTOCOL_SENSOR_VALUE;
    }

    if (format == PROTOCOL_SENSOR_FORMAT_RAW) {
        protocol_report_sensor_values_raw(kind, response);
        return;
    }

    KeyMatrixU8 matrix;
    memset(&matrix, 0, sizeof(matrix));

    if (kind == PROTOCOL_SENSOR_SCAN_ORDER) {
        protocol_fill_scan_order_matrix(&matrix);
    } else {
        protocol_fill_sensor_matrix(&matrix, kind);
    }

    protocol_reply_message(response, &KeyMatrixU8_msg, &matrix);
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_KEY_SENSOR_VALUES, protocol_cmd_get_key_sensor_values);

/* OUT-report command 21. Payload: ActuationPoints message; an empty payload resets every key
 * to "no own actuation point". With the persist flag the profile file "act" is written (or
 * removed for an empty payload). No response. */
static void protocol_cmd_set_actuation_points(const uint8_t *request)
{
    const protocol_report_profile_t *report = (const protocol_report_profile_t *)request;
    uint16_t length = report->length;
    uint16_t profile_id = protocol_report_profile_id(report->flags);
    ActuationPoints actuation_points;

    if (length != 0) {
        pb_istream_t stream = pb_istream_from_buffer(report->payload, length);

        if (!pb_decode(&stream, &ActuationPoints_msg, &actuation_points)) {
            return;
        }
        keys_sanitize_actuation_map((void *)&actuation_points);

        if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
            storage_write_profile_pb_file(profile_id, "act", &ActuationPoints_msg, ActuationPoints_size,
                                          &actuation_points);
        }
    } else if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && g_storage_mounted != 0) {
        char path[PROTOCOL_PROFILE_PATH_SIZE];

        storage_format_profile_path(path, profile_id, "act");
        lfs_remove(&g_storage_lfs, path);
        storage_bump_profile_revision(profile_id);
    }

    /* Without the persist bit the request is a live preview; a stored change reaches the RAM
     * state only when it addresses the active profile. */
    if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) && !protocol_profile_is_active(profile_id)) {
        return;
    }

    if (length != 0) {
        keys_apply_actuation_map((const void *)&actuation_points);
    } else {
        for (uint32_t pos = 0; pos != ANALOG_KEY_COUNT; ++pos) {
            g_analog_key_states[pos].actuation_point = KEYS_DISTANCE_UNSET;
        }
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_ACTUATION_POINTS, protocol_cmd_set_actuation_points);

/* Builds the ActuationPoints message from the key records. Returns false when there is
 * nothing to report (sparse form without entries). */
static bool protocol_export_actuation_points(ActuationPoints *actuation_points)
{
    uint8_t override_count = g_keys_per_key_actuation_count;
    bool dense = override_count > KEYS_LEGACY_MAP_THRESHOLD;

    /* KNOWN ISSUE: the message is not cleared. In the dense form the cells of keys without an
     * override (and columns 17..20) keep whatever was on the stack. Matches stock
     * firmware 2.14.1. */
    if (dense) {
        actuation_points->which_map = ActuationPoints_matrix_u16_tag;
    } else {
        actuation_points->map.list_u16.items_count = 0;
        actuation_points->which_map = ActuationPoints_list_u16_tag;
    }

    for (uint32_t row = 0; row != LAYOUT_ROWS; ++row) {
        for (uint32_t col = 0; col != LAYOUT_COLS; ++col) {
            uint8_t pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];
            if (pos == LAYOUT_NO_KEY) {
                continue;
            }
            uint16_t actuation_point = g_analog_key_states[pos].actuation_point;
            if (actuation_point == KEYS_DISTANCE_UNSET) {
                continue;
            }

            if (dense) {
                actuation_points->map.matrix_u16.rows[row].values[col] = actuation_point;
            } else {
                KeyValueListU16 *list = &actuation_points->map.list_u16;
                uint8_t *item = (uint8_t *)&list->items[list->items_count];

                /* KNOWN ISSUE: item = key id | value << 16, but only byte 0 and bytes 2..3 are
                 * stored; byte 1 is uninitialised stack and is sent to the host inside the
                 * varint. Matches stock firmware 2.14.1. */
                item[0] = KEYS_KEY_ID(row, col);
                item[2] = (uint8_t)actuation_point;
                item[3] = (uint8_t)(actuation_point >> 8);
                list->items_count++;
            }
        }
    }

    return dense || actuation_points->map.list_u16.items_count != 0;
}

/* Feature command 49. arg bits 15:0 = profile id, 0xFFFF = the live values. Reply: the
 * ActuationPoints message (from the profile file "act" for a stored profile); empty payload
 * if nothing is set or the file does not exist. */
static void protocol_cmd_get_actuation_points(uint32_t arg, protocol_response_t *response)
{
    uint16_t profile_id = (uint16_t)arg;
    ActuationPoints actuation_points;

    if (profile_id == PROFILE_ID_NONE) {
        if (!protocol_export_actuation_points(&actuation_points)) {
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
        storage_format_profile_path(path, profile_id, "act");
        result = storage_read_pb_file(path, &ActuationPoints_msg, &actuation_points);
        if (result == LFS_ERR_NOENT) {
            protocol_reply_empty(response);
            return;
        }
        if (result != 1) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
        keys_sanitize_actuation_map((void *)&actuation_points);
    }

    protocol_reply_message(response, &ActuationPoints_msg, &actuation_points);
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_ACTUATION_POINTS, protocol_cmd_get_actuation_points);
