/*
 * protocol_rgb_settings.c - host commands for the RGB settings of a profile (file `rgb`):
 * brightness, lock-indicator colours and the effect layers.
 *
 *   feature command  8     protocol_cmd_save_live_rgb     store the live settings and base colours
 *   feature command 50     protocol_cmd_get_rgb_settings  read the live or a stored RgbSettings
 *   OUT-report command 22  protocol_cmd_set_rgb_settings  replace the live RgbSettings
 */
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "pb_decode.h"
#include "pb_encode.h"

#include "core/run_state.h"
#include "proto/settings.pb.h"
#include "profile/profile_data.h"
#include "protocol/protocol.h"
#include "rgb/rgb_effects.h"
#include "rgb/rgb_frame.h"
#include "storage/storage.h"

/* Size limit of the encoded file `rgb`; matches stock firmware 2.14.1. The generated
 * RgbSettings_size is 200: the schema in proto/ allows 18 bytes more than this limit. */
#define PROTOCOL_RGB_FILE_BUFFER_SIZE 182u

/* Path buffer of the profile files with a three-letter name ("lp/255/rgb" + NUL fits). */
#define PROTOCOL_RGB_FILE_PATH_SIZE 12u

/*
 * A profile id that names a directory: built-in profile 0..3 or `lp` profile 0..9.
 */
static bool protocol_profile_id_has_directory(uint16_t profile_id)
{
    if (profile_id == PROFILE_ID_NONE) {
        return false;
    }
    if (PROFILE_ID_NS(profile_id) == PROFILE_NS_BUILTIN) {
        return PROFILE_ID_INDEX(profile_id) < PROFILE_BUILTIN_MAX;
    }
    if (PROFILE_ID_NS(profile_id) == PROFILE_NS_LP) {
        return PROFILE_ID_INDEX(profile_id) < PROFILE_LP_MAX;
    }
    return false;
}

/*
 * Feature command 8: stores the live RGB settings (file `rgb`) and the live base colours (file
 * `clr`) in the profile given by arg bits 15:0. No payload.
 */
static void protocol_cmd_save_live_rgb(uint32_t arg, protocol_response_t *response)
{
    uint16_t profile_id = (uint16_t)arg;

    if (!protocol_profile_id_has_directory(profile_id)) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    if (g_storage_mounted != 0) {
        storage_write_profile_pb_file(profile_id, "rgb", &RgbSettings_msg, PROTOCOL_RGB_FILE_BUFFER_SIZE,
                                      &g_rgb_settings);
    }
    /* storage_write_base_colors checks g_storage_mounted itself. */
    storage_write_base_colors(profile_id, g_rgb_base_colors);

    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_SAVE_LIVE_RGB, protocol_cmd_save_live_rgb);

/*
 * Feature command 50: returns an RgbSettings message. arg bits 15:0 = profile id; 0xFFFF = the
 * live settings, otherwise file `rgb` of that profile read over the message defaults.
 */
static void protocol_cmd_get_rgb_settings(uint32_t arg, protocol_response_t *response)
{
    pb_ostream_t stream = pb_ostream_from_buffer(response->payload, PROTOCOL_RESPONSE_PAYLOAD);
    uint16_t profile_id = (uint16_t)arg;

    if (profile_id == PROFILE_ID_NONE) {
        if (!pb_encode(&stream, &RgbSettings_msg, &g_rgb_settings)) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
    } else {
        RgbSettings settings;

        /* RgbSettings_init_default: brightness 255, the four indicator colours 0xFFFF. */
        memset(&settings, 0, sizeof(settings));
        settings.base = (RgbBaseSettings)RgbBaseSettings_init_default;

        if (g_storage_mounted != 0) {
            char path[PROTOCOL_RGB_FILE_PATH_SIZE];

            storage_format_profile_path(path, profile_id, "rgb");
            /* KNOWN ISSUE: storage_read_pb_file returns 1 or a negative error, never 0, so a
             * missing or damaged file is not reported: the host gets the defaults (or a partly
             * decoded message) with status OK. Matches stock firmware 2.14.1. */
            if (storage_read_pb_file(path, &RgbSettings_msg, &settings) == 0) {
                response->status = PROTOCOL_STATUS_ERROR;
                return;
            }
        }
        if (!pb_encode(&stream, &RgbSettings_msg, &settings)) {
            response->status = PROTOCOL_STATUS_ERROR;
            return;
        }
    }

    response->length = (uint16_t)stream.bytes_written;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_RGB_SETTINGS, protocol_cmd_get_rgb_settings);

/*
 * OUT-report command 22: replaces the RGB settings of the active profile with the RgbSettings
 * message of the report (decoded straight into g_rgb_settings, so a decode error leaves a partly
 * overwritten struct and nothing else happens). Flag bit 0 also writes file `rgb`.
 */
static void protocol_cmd_set_rgb_settings(const uint8_t *report)
{
    const protocol_report_profile_t *request = (const protocol_report_profile_t *)report;
    pb_istream_t stream = pb_istream_from_buffer(request->payload, request->length);

    if (!pb_decode(&stream, &RgbSettings_msg, &g_rgb_settings)) {
        return;
    }

    if ((request->flags & PROTOCOL_REPORT_FLAG_PERSIST) != 0 && g_storage_mounted != 0) {
        storage_write_profile_pb_file(g_profile_active_id.raw, "rgb", &RgbSettings_msg,
                                      PROTOCOL_RGB_FILE_BUFFER_SIZE, &g_rgb_settings);
    }

    rgb_reset_idle_timer();

    if (g_rgb_settings.which_effect == RgbSettings_legacy_effect_tag) {
        rgb_settings_convert_legacy_effect();
        rgb_effects_apply_layer_settings();
    } else if (g_rgb_settings.which_effect == RgbSettings_layers_tag) {
        rgb_effects_apply_layer_settings();
    } else {
        g_rgb_settings.effect.layers.layers_count = 0;
        g_rgb_effect_layer_count = 0;
    }

    /* The base colours are repainted unless effect layers own the key LEDs (scan mode level 0 and
     * at least one runtime layer). */
    if ((g_run_state_flags >> RUN_STATE_SCAN_MODE_SHIFT) != 0 || g_rgb_effect_layer_count == 0) {
        rgb_paint_base_colors(0);
    }
    rgb_apply_brightness_change();
    rgb_update_lock_indicators();
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SET_RGB_SETTINGS, protocol_cmd_set_rgb_settings);
