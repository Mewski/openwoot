/*
 * lightbar_protocol.c - host commands of feature `lightbar`:
 *
 *   feature command 66     protocol_cmd_get_lightbar_config          LightbarConfig (file `lb`)
 *   OUT-report command 32  protocol_cmd_set_lightbar_config
 *   feature command 67     protocol_cmd_get_global_lightbar_config   GlobalLightbarConfig (file `glb`)
 *   OUT-report command 33  protocol_cmd_set_global_lightbar_config
 *   feature command 68     protocol_cmd_get_lightbar_colors          LightbarColors read-back
 *   OUT-report command 30  protocol_report_set_lightbar_colors       host-driven light-bar overlay
 *   feature command 70     protocol_cmd_get_lbm_file                 LbmFile (file `lbm` of a profile)
 *   OUT-report command 34  protocol_report_save_lbm
 *   feature command 71     protocol_cmd_get_glbm                     GlbmFile (root file `glbm`)
 *   OUT-report command 35  protocol_cmd_save_glbm
 *   event 2                protocol_send_lightbar_changed_event
 *
 * The files `lbm` and `glbm` are kept for the host only; the firmware does not read them.
 *
 * Depends on the protocol module (registration, report layouts, events), the profile and
 * storage modules and nanopb.
 */
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "lfs.h"
#include "pb_decode.h"
#include "pb_encode.h"

#include "profile/profile_data.h"
#include "protocol/protocol.h"
#include "rgb/lightbar/lightbar_internal.h"
#include "rgb/rgb_frame.h"
#include "storage/storage.h"

/* Path buffer of the profile files with a two- or three-letter name ("lp/255/lbm" + NUL fits). */
#define PROTOCOL_LIGHTBAR_FILE_PATH_SIZE 12u

/* Encoded size limits of the files `lbm` and `glbm` (buffers on the stack). */
#define PROTOCOL_LBM_FILE_MAX      136u
#define PROTOCOL_GLBM_FILE_MAX     136u

_Static_assert(sizeof(((LightbarColors *)0)->colors) / sizeof(RgbColor) == LIGHTBAR_LED_COUNT,
               "LightbarColors holds one colour per light-bar LED");

/* ---- Light-bar configuration ------------------------------------------------------------------- */

/*
 * Feature command 66: returns a LightbarConfig message. arg bits 15:0 = profile id; 0xFFFF = the
 * live configuration, otherwise file `lb` of that profile (a missing file answers OK with
 * length 0).
 */
static void protocol_cmd_get_lightbar_config(uint32_t arg, protocol_response_t *response)
{
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);
    uint16_t profile_id = (uint16_t)arg;

    if (profile_id == PROFILE_ID_NONE) {
        if (!pb_encode(&stream, &LightbarConfig_msg, &g_rgb_lightbar_config)) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
    } else {
        LightbarConfig config;
        char path[PROTOCOL_LIGHTBAR_FILE_PATH_SIZE];
        int result;

        /* LightbarConfig_init_default */
        memset(&config, 0, sizeof(config));
        config.enabled = true;
        config.use_global = true;
        config.brightness = 255;

        if (g_storage_mounted == 0) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
        storage_format_profile_path(path, profile_id, "lb");
        result = storage_read_pb_file(path, &LightbarConfig_msg, &config);
        if (result != STORAGE_OK) {
            if (result == LFS_ERR_NOENT) {
                response->length = 0;
                response->status = PROTOCOL_STATUS_OK;
            } else {
                response->status = PROTOCOL_STATUS_ERROR;
            }
            return;
        }
        if (!pb_encode(&stream, &LightbarConfig_msg, &config)) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
    }

    response->length = (uint16_t)stream.bytes_written;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_LIGHTBAR_CONFIG, protocol_cmd_get_lightbar_config);

/*
 * OUT-report command 32: sets the light-bar configuration of a profile. Flag bit 0 writes file
 * `lb` of the profile named by the flags byte; the live configuration is replaced when the
 * report is not persistent or when it targets the active profile. No response.
 */
static void protocol_cmd_set_lightbar_config(const uint8_t *report)
{
    const protocol_report_profile_t *request = (const protocol_report_profile_t *)report;
    uint8_t index = (request->flags & PROTOCOL_REPORT_FLAG_INDEX_MASK) >> PROTOCOL_REPORT_FLAG_INDEX_SHIFT;
    uint8_t ns = request->flags >> PROTOCOL_REPORT_FLAG_NS_SHIFT;
    pb_istream_t stream = pb_istream_from_buffer(request->payload, request->length);
    LightbarConfig config;

    if (!pb_decode(&stream, &LightbarConfig_msg, &config)) {
        return;
    }

    if ((request->flags & PROTOCOL_REPORT_FLAG_PERSIST) != 0 && g_storage_mounted != 0) {
        storage_write_profile_pb_file(PROFILE_ID_RAW(index, ns), "lb", &LightbarConfig_msg, LightbarConfig_size,
                                      &config);
    }

    if ((request->flags & PROTOCOL_REPORT_FLAG_PERSIST) == 0 ||
        (index == g_profile_active_id.index && g_profile_active_id.ns == ns)) {
        g_rgb_lightbar_config = config;
        lightbar_config_changed();
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_LIGHTBAR_CONFIG, protocol_cmd_set_lightbar_config);

/*
 * Feature command 67: returns the keyboard-wide light-bar entries. arg == 0 reads file `glb` (a
 * missing file answers OK with length 0).
 */
static void protocol_cmd_get_global_lightbar_config(uint32_t arg, protocol_response_t *response)
{
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);

    if (arg != 0) {
        /* KNOWN ISSUE: the "live" variant encodes the profile configuration g_rgb_lightbar_config
         * (LightbarConfig: enabled @0, entry count @2) with the GlobalLightbarConfig descriptor
         * (entry count @0), so the entry count sent is the `enabled` byte. The live copy of file
         * `glb` is g_rgb_global_lightbar_config. Matches stock firmware 2.14.1. */
        if (!pb_encode(&stream, &GlobalLightbarConfig_msg, &g_rgb_lightbar_config)) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
    } else {
        GlobalLightbarConfig config;
        int result;

        memset(&config, 0, sizeof(config));
        if (g_storage_mounted == 0) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
        result = storage_read_pb_file("glb", &GlobalLightbarConfig_msg, &config);
        if (result != STORAGE_OK) {
            if (result == LFS_ERR_NOENT) {
                response->length = 0;
                response->status = PROTOCOL_STATUS_OK;
            } else {
                response->status = PROTOCOL_STATUS_ERROR;
            }
            return;
        }
        if (!pb_encode(&stream, &GlobalLightbarConfig_msg, &config)) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
    }

    response->length = (uint16_t)stream.bytes_written;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_GLOBAL_LIGHTBAR_CONFIG, protocol_cmd_get_global_lightbar_config);

/*
 * OUT-report command 33: sets the keyboard-wide light-bar entries; flag bit 0 also writes file
 * `glb`. The live copy is always replaced. No response.
 */
static void protocol_cmd_set_global_lightbar_config(const uint8_t *report)
{
    const protocol_report_profile_t *request = (const protocol_report_profile_t *)report;
    pb_istream_t stream = pb_istream_from_buffer(request->payload, request->length);
    GlobalLightbarConfig config;

    if (!pb_decode(&stream, &GlobalLightbarConfig_msg, &config)) {
        return;
    }

    if ((request->flags & PROTOCOL_REPORT_FLAG_PERSIST) != 0 && g_storage_mounted != 0) {
        lightbar_write_global_file(&config);
    }

    g_rgb_global_lightbar_config = config;
    lightbar_config_changed();
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_GLOBAL_LIGHTBAR_CONFIG, protocol_cmd_set_global_lightbar_config);

void protocol_send_lightbar_changed_event(void)
{
    Event event;

    memset(&event, 0, sizeof(event));
    event.which_event = Event_lightbar_changed_tag;

    if (g_protocol_event_subscriptions[PROTOCOL_EVENT_LIGHTBAR_CHANGED] != 0) {
        protocol_send_event(&event);
    }
}

/* ---- Light-bar colours ------------------------------------------------------------------------- */

/*
 * OUT-report command 30: the host drives the light bar directly. The LightbarColors message
 * gives the colour of each light-bar position; they are shown by overlay 0x1D until a report
 * with payload length 0 stops it. Ignored while the RGB SDK owns the LEDs; a decode error is
 * ignored as well. No response.
 */
static void protocol_report_set_lightbar_colors(const uint8_t *report)
{
    const protocol_report_plain_t *request = (const protocol_report_plain_t *)report;
    uint16_t length = request->length;
    LightbarColors colors;
    pb_istream_t stream;
    rgb_overlay_params_t params;

    if (length == 0) {
        rgb_overlay_stop(RGB_OVERLAY_ID_HOST_LIGHTBAR);
        return;
    }

    memset(&colors, 0, sizeof(colors));
    stream = pb_istream_from_buffer(request->payload, length);
    if (!pb_decode(&stream, &LightbarColors_msg, &colors)) {
        return;
    }

    memset(&params, 0, sizeof(params));
    params.type = RGB_OVERLAY_LIGHTBAR_COLORS;
    for (uint32_t i = 0; i < colors.colors_count && i < LIGHTBAR_LED_COUNT; i++) {
        params.u.lightbar.colors[i].r = colors.colors[i].r;
        params.u.lightbar.colors[i].g = colors.colors[i].g;
        params.u.lightbar.colors[i].b = colors.colors[i].b;
    }

    if (g_rgb_sdk_mode_active == 0) {
        rgb_overlay_start(RGB_OVERLAY_ID_HOST_LIGHTBAR, RGB_OVERLAY_DURATION_FOREVER, &params);
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_LIGHTBAR_COLORS, protocol_report_set_lightbar_colors);

/*
 * Feature command 68: returns the colours of the light-bar LEDs as they stand in the main frame
 * (before brightness and gamma). An LED the frame cannot locate reads as black. arg is unused.
 */
static void protocol_cmd_get_lightbar_colors(uint32_t arg, protocol_response_t *response)
{
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);
    LightbarColors colors;

    memset(&colors, 0, sizeof(colors));
    colors.colors_count = LIGHTBAR_LED_COUNT;

    for (uint32_t i = 0; i < LIGHTBAR_LED_COUNT; i++) {
        uint8_t page_r;
        uint8_t page_g;
        uint8_t page_b;
        uint8_t *ch_r;
        uint8_t *ch_g;
        uint8_t *ch_b;

        if (rgb_frame_locate_led(&g_rgb_frame_main, k_lightbar_led_ids[i], &page_r, &page_g, &page_b, &ch_r,
                                 &ch_g, &ch_b) != 0) {
            colors.colors[i].r = *ch_r;
            colors.colors[i].g = *ch_g;
            colors.colors[i].b = *ch_b;
        }
    }

    if (pb_encode(&stream, &LightbarColors_msg, &colors)) {
        response->length = (uint16_t)stream.bytes_written;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->length = 0;
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_LIGHTBAR_COLORS, protocol_cmd_get_lightbar_colors);

/* ---- Files kept for the host: `lbm`, `glbm` ------------------------------------------------------ */

/* Feature command 70. arg bits 15:0 = profile id. Reply: the LbmFile message of the file
 * "lbm"; empty payload if the file does not exist. The message has no RAM copy, so id
 * 0xFFFF is rejected. */
static void protocol_cmd_get_lbm_file(uint32_t arg, protocol_response_t *response)
{
    uint16_t profile_id = (uint16_t)arg;
    char path[PROTOCOL_LIGHTBAR_FILE_PATH_SIZE];
    LbmFile message;

    if (profile_id == PROFILE_ID_NONE) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);

    memset(&message, 0, sizeof(message));

    if (g_storage_mounted == 0u) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }
    storage_format_profile_path(path, profile_id, "lbm");

    int result = storage_read_pb_file(path, &LbmFile_msg, &message);

    if (result == LFS_ERR_NOENT) {
        response->length = 0u;
        response->status = PROTOCOL_STATUS_OK;
    } else if (result == STORAGE_OK && pb_encode(&stream, &LbmFile_msg, &message)) {
        response->length = (uint16_t)stream.bytes_written;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_LBM_FILE, protocol_cmd_get_lbm_file);

/* OUT-report command 34. Writes the LbmFile message to the file "lbm" of the addressed
 * profile. The message has no RAM copy, so a request without the persist flag does nothing.
 * No response. */
static void protocol_report_save_lbm(const uint8_t *report)
{
    const protocol_report_profile_t *request = (const protocol_report_profile_t *)report;
    uint8_t flags = request->flags;

    if ((flags & PROTOCOL_REPORT_FLAG_PERSIST) == 0u) {
        return;
    }

    LbmFile message;
    pb_istream_t stream = pb_istream_from_buffer(request->payload, request->length);

    if (pb_decode(&stream, &LbmFile_msg, &message) && g_storage_mounted != 0u) {
        storage_write_profile_pb_file(protocol_report_profile_id(flags), "lbm", &LbmFile_msg,
                                      PROTOCOL_LBM_FILE_MAX, &message);
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SAVE_LBM, protocol_report_save_lbm);

/* Feature command 71. Reply: the GlbmFile message read from the root file "glbm"; empty
 * payload if the file does not exist. */
static void protocol_cmd_get_glbm(uint32_t arg, protocol_response_t *response)
{
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);
    GlbmFile message;

    memset(&message, 0, sizeof(message));

    if (g_storage_mounted == 0u) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    int result = storage_read_pb_file("glbm", &GlbmFile_msg, &message);

    if (result == LFS_ERR_NOENT) {
        response->length = 0u;
        response->status = PROTOCOL_STATUS_OK;
    } else if (result == STORAGE_OK && pb_encode(&stream, &GlbmFile_msg, &message)) {
        response->length = (uint16_t)stream.bytes_written;
        response->status = PROTOCOL_STATUS_OK;
    } else {
        response->status = PROTOCOL_STATUS_ERROR;
    }
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_GLBM, protocol_cmd_get_glbm);

/* OUT-report command 35. Writes the GlbmFile message to the root file "glbm". The message
 * has no RAM copy, so a request without the persist flag does nothing. No response. */
static void protocol_cmd_save_glbm(const uint8_t *request)
{
    const protocol_report_profile_t *report = (const protocol_report_profile_t *)request;

    if ((report->flags & PROTOCOL_REPORT_FLAG_PERSIST) == 0u) {
        return;
    }

    GlbmFile message;
    pb_istream_t stream = pb_istream_from_buffer(report->payload, report->length);

    if (pb_decode(&stream, &GlbmFile_msg, &message) && g_storage_mounted != 0u) {
        uint8_t encoded[PROTOCOL_GLBM_FILE_MAX];
        pb_ostream_t file_stream = pb_ostream_from_buffer(encoded, sizeof(encoded));

        /* The result of the encoder is not checked. */
        pb_encode(&file_stream, &GlbmFile_msg, &message);
        storage_write_file("glbm", encoded, (uint16_t)file_stream.bytes_written);
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SAVE_GLBM, protocol_cmd_save_glbm);
