/*
 * protocol_rgb_colors.c - host commands for the per-key colour maps of a profile.
 *
 * Base colours (file `clr`, g_rgb_base_colors): 6 rows x 21 RGB565 words, of which columns
 * 0..16 are keys. They travel as two ColorRows messages of three rows each:
 *   feature commands 35 / 36     protocol_cmd_get_base_colors_rows_0_2 / _rows_3_5
 *   OUT-report commands 14 / 15  protocol_cmd_set_base_colors_rows0_2 / _rows3_5
 *
 * Layer colours (files `r1`..`r3`, g_rgb_layer_colors): 6 rows x 17 RGB565 words per Fn layer:
 *   feature command 57           protocol_cmd_get_layer_colors (one half per call, as ColorRows)
 *   OUT-report command 27        protocol_cmd_set_layer_colors (raw 6 x 21 words, or delete)
 */
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "pb_decode.h"
#include "pb_encode.h"

#include "proto/settings.pb.h"
#include "profile/profile_data.h"
#include "protocol/protocol.h"
#include "rgb/rgb_frame.h"
#include "storage/storage.h"

/* Rows carried by one ColorRows message: half of a colour map. */
#define PROTOCOL_COLOR_ROWS_PER_MESSAGE 3u

_Static_assert(sizeof(ColorRows) == PROTOCOL_COLOR_ROWS_PER_MESSAGE * sizeof(g_rgb_base_colors[0]),
               "ColorRows is three rows of g_rgb_base_colors");
_Static_assert(sizeof(g_rgb_base_colors) == STORAGE_BASE_COLORS_SIZE, "g_rgb_base_colors is file clr");
_Static_assert(sizeof(g_rgb_layer_colors[0]) == STORAGE_LAYER_COLORS_SIZE, "layer colour map is file r<k>");

/* ---- Base colours ------------------------------------------------------------------------------ */

/*
 * Reads file `clr` of a profile. Returns STORAGE_OK, STORAGE_ERR_NOT_MOUNTED or the littlefs
 * error truncated to 16 bits; on any error `colors` keeps its content.
 */
static int16_t protocol_read_base_colors_file(uint16_t profile_id,
                                              uint16_t colors[RGB_MATRIX_ROWS][RGB_BASE_COLOR_COLS])
{
    char path[28];
    int read;

    if (g_storage_mounted == 0) {
        return STORAGE_ERR_NOT_MOUNTED;
    }
    storage_format_profile_path(path, profile_id, "clr");
    read = storage_read_file(path, colors, STORAGE_BASE_COLORS_SIZE);
    if (read < 0) {
        return (int16_t)read;
    }
    return STORAGE_OK;
}

/*
 * Answers with rows first_row .. first_row + 2 of a base colour map. arg bits 15:0 = profile
 * id; 0xFFFF = the live map, otherwise file `clr` of that profile (all zero if it cannot be read).
 */
static void protocol_send_base_color_rows(uint32_t arg, protocol_response_t *response, uint32_t first_row)
{
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);
    uint16_t profile_id = (uint16_t)arg;

    if (profile_id == PROFILE_ID_NONE) {
        if (!pb_encode(&stream, &ColorRows_msg, g_rgb_base_colors[first_row])) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
    } else {
        uint16_t colors[RGB_MATRIX_ROWS][RGB_BASE_COLOR_COLS];

        memset(colors, 0, sizeof(colors));
        /* KNOWN ISSUE: the read result is 1 or negative, so this test never fires (it would need
         * a littlefs error whose low 16 bits are zero): an unmounted filesystem, a missing file
         * or an invalid profile id answer with an all-zero map and status OK. Matches stock
         * firmware 2.14.1. */
        if (protocol_read_base_colors_file(profile_id, colors) == 0) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
        if (!pb_encode(&stream, &ColorRows_msg, colors[first_row])) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
    }

    response->length = (uint16_t)stream.bytes_written;
    response->status = PROTOCOL_STATUS_OK;
}

/* Feature command 35: rows 0..2 of the base colour map. arg bits 15:0 = profile id, 0xFFFF =
 * the live map. Reply: ColorRows message. */
static void protocol_cmd_get_base_colors_rows_0_2(uint32_t arg, protocol_response_t *response)
{
    protocol_send_base_color_rows(arg, response, 0);
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_BASE_COLORS_ROWS_0_2, protocol_cmd_get_base_colors_rows_0_2);

/* Feature command 36: rows 3..5 of the base colour map. arg bits 15:0 = profile id, 0xFFFF =
 * the live map. Reply: ColorRows message. */
static void protocol_cmd_get_base_colors_rows_3_5(uint32_t arg, protocol_response_t *response)
{
    protocol_send_base_color_rows(arg, response, PROTOCOL_COLOR_ROWS_PER_MESSAGE);
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_BASE_COLORS_ROWS_3_5, protocol_cmd_get_base_colors_rows_3_5);

/*
 * Decodes the ColorRows message of the report straight into rows first_row .. first_row + 2 of
 * the live base colour map (a decode error leaves them partly overwritten and nothing else
 * happens). Flag bit 0 also writes the whole map to file `clr` of the active profile.
 */
static void protocol_receive_base_color_rows(const uint8_t *report, uint32_t first_row)
{
    const protocol_report_profile_t *request = (const protocol_report_profile_t *)report;
    pb_istream_t stream = pb_istream_from_buffer(request->payload, request->length);

    if (!pb_decode(&stream, &ColorRows_msg, g_rgb_base_colors[first_row])) {
        return;
    }

    if ((request->flags & PROTOCOL_REPORT_FLAG_PERSIST) != 0) {
        storage_write_base_colors(g_profile_active_id.raw, g_rgb_base_colors);
    }
    rgb_reset_idle_timer();
    rgb_paint_base_colors(0);
}

/* OUT-report command 14: rows 0..2 of the base colour map. Payload: ColorRows message. No
 * response. */
static void protocol_cmd_set_base_colors_rows0_2(const uint8_t *report)
{
    protocol_receive_base_color_rows(report, 0);
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_BASE_COLORS_ROWS0_2, protocol_cmd_set_base_colors_rows0_2);

/* OUT-report command 15: rows 3..5 of the base colour map. Payload: ColorRows message. No
 * response. */
static void protocol_cmd_set_base_colors_rows3_5(const uint8_t *report)
{
    protocol_receive_base_color_rows(report, PROTOCOL_COLOR_ROWS_PER_MESSAGE);
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_BASE_COLORS_ROWS3_5, protocol_cmd_set_base_colors_rows3_5);

/* ---- Layer colours ----------------------------------------------------------------------------- */

/* OUT report of command 27. The payload is raw, not protobuf: a whole colour map in the 21-column
 * row format of file `clr`, of which the first 17 columns of every row are stored. */
typedef struct __attribute__((packed)) protocol_report_layer_colors {
    uint16_t magic;                                          /* +0x00 */
    uint8_t  cmd;                                            /* +0x02 */
    uint8_t  flags;                                          /* +0x03 PROTOCOL_LAYER_COLORS_* */
    uint16_t colors[RGB_MATRIX_ROWS][RGB_BASE_COLOR_COLS];   /* +0x04 RGB565 */
} protocol_report_layer_colors_t;

_Static_assert(sizeof(protocol_report_layer_colors_t) == 0x100, "protocol_report_layer_colors_t size");
_Static_assert(offsetof(protocol_report_layer_colors_t, colors) == 4, "protocol_report_layer_colors_t.colors");

/* protocol_report_layer_colors_t.flags */
#define PROTOCOL_LAYER_COLORS_WRITE        0x01u  /* bit 0: 1 = store the map, 0 = delete the file */
#define PROTOCOL_LAYER_COLORS_INDEX_SHIFT  1u     /* bits 4:1 profile index */
#define PROTOCOL_LAYER_COLORS_INDEX_MASK   0x0Fu
#define PROTOCOL_LAYER_COLORS_LAYER_SHIFT  5u     /* bits 7:5 layer, 1..3 expected */

/* The last word of the map (row 5, column 20: never a key) carries the profile namespace:
 * high byte 0xAE marks it as valid, the low byte is the namespace. */
#define PROTOCOL_LAYER_COLORS_NS_ROW       (RGB_MATRIX_ROWS - 1)
#define PROTOCOL_LAYER_COLORS_NS_COL       (RGB_BASE_COLOR_COLS - 1)
#define PROTOCOL_LAYER_COLORS_NS_MARKER    0xAEu

_Static_assert(offsetof(protocol_report_layer_colors_t,
                        colors[PROTOCOL_LAYER_COLORS_NS_ROW][PROTOCOL_LAYER_COLORS_NS_COL]) == 0xFE,
               "namespace marker of command 27");

/*
 * Removes file "<profile dir>/r<layer>" and bumps the profile revision. Counterpart of
 * storage_write_layer_colors. The caller passes namespace 0 or 1 only.
 */
static void protocol_delete_layer_colors_file(uint16_t profile_id, uint32_t layer)
{
    char dir[8];
    char name[16];
    char path[28];

    if (g_storage_mounted == 0) {
        return;
    }
    sprintf(name, "r%d", (int)layer);
    if (PROFILE_ID_NS(profile_id) == PROFILE_NS_LP) {
        sprintf(dir, "lp/%d", PROFILE_ID_INDEX(profile_id));
    } else {
        sprintf(dir, "%d", PROFILE_ID_INDEX(profile_id));
    }
    sprintf(path, "%s/%s", dir, name);
    storage_bump_profile_revision(profile_id);
    lfs_remove(&g_storage_lfs, path);
}

/*
 * OUT-report command 27: stores or deletes the colour map of one Fn layer of a profile and, if
 * that profile is the active one, updates the live maps. No response.
 */
static void protocol_cmd_set_layer_colors(const uint8_t *request)
{
    const protocol_report_layer_colors_t *report = (const protocol_report_layer_colors_t *)request;
    uint16_t ns_word = report->colors[PROTOCOL_LAYER_COLORS_NS_ROW][PROTOCOL_LAYER_COLORS_NS_COL];
    uint8_t ns;
    uint8_t flags;
    uint8_t index;
    bool is_active_profile;

    if ((ns_word >> 8) == PROTOCOL_LAYER_COLORS_NS_MARKER) {
        ns = (uint8_t)ns_word;
        if (ns > PROFILE_NS_LP) {
            return;
        }
    } else {
        ns = PROFILE_NS_BUILTIN;
    }

    flags = report->flags;
    index = (flags >> PROTOCOL_LAYER_COLORS_INDEX_SHIFT) & PROTOCOL_LAYER_COLORS_INDEX_MASK;
    is_active_profile = g_profile_active_id.index == index && ns == g_profile_active_id.ns;

    if ((flags & PROTOCOL_LAYER_COLORS_WRITE) != 0) {
        uint16_t colors[RGB_MATRIX_ROWS][RGB_MATRIX_COLS];
        uint8_t layer;

        for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
            memcpy(colors[row], report->colors[row], sizeof(colors[row]));
        }
        storage_write_layer_colors(PROFILE_ID_RAW(index, ns), flags >> PROTOCOL_LAYER_COLORS_LAYER_SHIFT, colors);

        if (is_active_profile) {
            /* KNOWN ISSUE: the layer is not range checked. Layer 0 writes the 204 bytes in front
             * of g_rgb_layer_colors, layers 4..7 behind it. Matches stock firmware 2.14.1. */
            memcpy(g_rgb_layer_colors[(report->flags >> PROTOCOL_LAYER_COLORS_LAYER_SHIFT) - 1], colors,
                   sizeof(g_rgb_layer_colors[0]));
            layer = report->flags >> PROTOCOL_LAYER_COLORS_LAYER_SHIFT;
            if (g_rgb_layer_color_count + 1 == layer) {
                g_rgb_layer_color_count = layer;
            }
        }
    } else {
        protocol_delete_layer_colors_file(PROFILE_ID_RAW(index, ns), flags >> PROTOCOL_LAYER_COLORS_LAYER_SHIFT);

        if (is_active_profile) {
            /* The maps are loaded as a run r1, r2, ...: deleting r<layer> ends the run before it.
             * KNOWN ISSUE: layer 0 stores -1 (255) as the count. Matches stock firmware 2.14.1. */
            int remaining = (report->flags >> PROTOCOL_LAYER_COLORS_LAYER_SHIFT) - 1;

            if (remaining >= g_rgb_layer_color_count) {
                remaining = g_rgb_layer_color_count;
            }
            g_rgb_layer_color_count = (uint8_t)remaining;
        }
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_LAYER_COLORS, protocol_cmd_set_layer_colors);

/*
 * Feature command 57: returns one half of the colour map of one Fn layer as a ColorRows message
 * (columns 17..20 zero). arg: byte 0 = profile index, byte 1 = layer, byte 2 = half (0 = rows
 * 0..2, else rows 3..5), byte 3 = profile namespace. A missing file answers OK with length 0.
 */
static void protocol_cmd_get_layer_colors(uint32_t arg, protocol_response_t *response)
{
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);
    uint16_t layer_colors[RGB_MATRIX_ROWS][RGB_MATRIX_COLS];
    ColorRows rows;
    uint32_t first_row;
    int result;

    result = storage_read_layer_colors(PROFILE_ID_RAW(arg, arg >> 24), (uint8_t)(arg >> 8), layer_colors);

    /* The rows are copied before the read result is looked at (stack garbage if the read failed;
     * it is not sent then). */
    memset(&rows, 0, sizeof(rows));
    first_row = (arg & 0x00FF0000u) != 0 ? PROTOCOL_COLOR_ROWS_PER_MESSAGE : 0;
    for (uint32_t row = 0; row < PROTOCOL_COLOR_ROWS_PER_MESSAGE; row++) {
        memcpy(rows.rows[row].rgb565, layer_colors[first_row + row], sizeof(layer_colors[0]));
    }

    if (result == STORAGE_OK) {
        if (pb_encode(&stream, &ColorRows_msg, &rows)) {
            response->length = (uint16_t)stream.bytes_written;
            response->status = PROTOCOL_STATUS_OK;
        } else {
            response->length = 0;
            response->status = PROTOCOL_STATUS_ERROR;
        }
    } else if (result == LFS_ERR_NOENT) {
        response->length = 0;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_LAYER_COLORS, protocol_cmd_get_layer_colors);
