/*
 * protocol_rgb_sdk.c - host commands that read or drive the key LEDs directly.
 *
 * Wooting RGB SDK commands (same command numbers as the public SDK): while the host drives the
 * LEDs, g_rgb_sdk_mode_active is set, which stops the lock indicators, the layer colour maps and
 * the effects.
 *   feature command 33     protocol_cmd_sdk_init_colors      enter direct mode
 *   feature command 30     protocol_cmd_sdk_set_key_color    one key
 *   OUT-report command 11  protocol_cmd_sdk_set_all_colors   whole matrix
 *   feature command 31     protocol_cmd_sdk_reset_key_color  one key back to its profile colour
 *   feature command 32     protocol_cmd_sdk_reset_colors     leave direct mode
 *
 * LED frame read-back:
 *   feature command 74     protocol_cmd_get_led_frame
 */
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "proto/settings.pb.h"
#include "protocol/protocol.h"
#include "rgb/lightbar/lightbar.h"
#include "rgb/rgb_frame.h"

/* ---- Wooting RGB SDK --------------------------------------------------------------------------- */

/* Key address of feature commands 30 (arg bits 31:24) and 31 (arg bits 7:0): row << 5 | column. */
#define PROTOCOL_SDK_KEY_ROW_SHIFT 5u
#define PROTOCOL_SDK_KEY_COL_MASK  0x1Fu

/* OUT report of command 11: a whole colour map in the 21-column row format of file `clr`,
 * starting at the odd offset 3. Columns 17..20 are ignored. */
typedef struct __attribute__((packed)) protocol_report_sdk_colors {
    uint16_t magic;                                          /* +0x00 */
    uint8_t  cmd;                                            /* +0x02 */
    uint16_t colors[RGB_MATRIX_ROWS][RGB_BASE_COLOR_COLS];   /* +0x03 RGB565 */
} protocol_report_sdk_colors_t;

_Static_assert(offsetof(protocol_report_sdk_colors_t, colors) == 3, "protocol_report_sdk_colors_t.colors");
_Static_assert(sizeof(protocol_report_sdk_colors_t) == 255, "protocol_report_sdk_colors_t size");

/*
 * Hands the key LEDs to the host if it does not own them yet: stops the lock indicators (they
 * see the mode flag), drops a shown or pending layer colour map and forces a render. In this
 * mode rgb_paint_base_colors(0) only raises the redraw flags.
 */
static void protocol_sdk_enter_direct_mode(void)
{
    if (g_rgb_sdk_mode_active != 0) {
        return;
    }
    g_rgb_sdk_mode_active = 1;
    rgb_update_lock_indicators();
    g_rgb_shown_color_layer = 0;
    g_rgb_layer_switch_pending = 0;
    rgb_paint_base_colors(0);
    g_rgb_force_render = 1;
}

/* Writes a 0xAARRGGBB colour to the LED of one key (no LED at that position: ignored). */
static void protocol_sdk_set_key_led(uint32_t row, uint32_t col, uint32_t argb)
{
    rgb_frame_set_led(&g_rgb_frame_main, g_rgb_key_to_led[row * RGB_MATRIX_COLS + col], (uint8_t)(argb >> 16),
                      (uint8_t)(argb >> 8), (uint8_t)argb);
}

/*
 * Feature command 30: sets the LED of one key. arg bits 31:29 = row, 28:24 = column, 23:16 =
 * red, 15:8 = green, 7:0 = blue. Enters direct mode first, also when the key address is invalid.
 */
static void protocol_cmd_sdk_set_key_color(uint32_t arg, protocol_response_t *response)
{
    uint32_t row;
    uint32_t col;

    protocol_sdk_enter_direct_mode();

    row = arg >> (24 + PROTOCOL_SDK_KEY_ROW_SHIFT);
    col = (arg >> 24) & PROTOCOL_SDK_KEY_COL_MASK;
    if (row >= RGB_MATRIX_ROWS || col >= RGB_MATRIX_COLS) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    rgb_frame_set_led(&g_rgb_frame_main, g_rgb_key_to_led[row * RGB_MATRIX_COLS + col], (uint8_t)(arg >> 16),
                      (uint8_t)(arg >> 8), (uint8_t)arg);
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_SDK_SET_KEY_COLOR, protocol_cmd_sdk_set_key_color);

/*
 * Feature command 31: gives one key LED its profile colour back (from the shown layer colour map
 * if there is one, else from the base colours). arg bits 7:5 = row, 4:0 = column. Does not
 * change the mode.
 */
static void protocol_cmd_sdk_reset_key_color(uint32_t arg, protocol_response_t *response)
{
    uint32_t row = (uint8_t)arg >> PROTOCOL_SDK_KEY_ROW_SHIFT;
    uint32_t col = arg & PROTOCOL_SDK_KEY_COL_MASK;
    uint16_t rgb565;

    if (row >= RGB_MATRIX_ROWS || col >= RGB_MATRIX_COLS) {
        response->status = PROTOCOL_STATUS_ERROR;
        return;
    }

    if (g_rgb_shown_color_layer != 0 && g_rgb_shown_layer_colors != NULL) {
        rgb565 = g_rgb_shown_layer_colors[row * RGB_MATRIX_COLS + col];
    } else {
        rgb565 = g_rgb_base_colors[row][col];
    }
    protocol_sdk_set_key_led(row, col, rgb_convert_rgb565_to_argb(rgb565));

    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_SDK_RESET_KEY_COLOR, protocol_cmd_sdk_reset_key_color);

/*
 * Feature command 32: leaves direct mode and repaints the profile colours. The lock indicators
 * are re-evaluated while the mode flag is still set (so they stay stopped until their next
 * update). arg is unused.
 */
static void protocol_cmd_sdk_reset_colors(uint32_t arg, protocol_response_t *response)
{
    protocol_sdk_enter_direct_mode();

    rgb_paint_base_colors(1);
    rgb_update_lock_indicators();
    response->status = PROTOCOL_STATUS_OK;
    g_rgb_sdk_mode_active = 0;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_SDK_RESET_COLORS, protocol_cmd_sdk_reset_colors);

/*
 * OUT-report command 11: paints all key LEDs from a 6 x 21 RGB565 map (columns 0..16 used).
 * Enters direct mode first. No response.
 */
static void protocol_cmd_sdk_set_all_colors(const uint8_t *report)
{
    const protocol_report_sdk_colors_t *request = (const protocol_report_sdk_colors_t *)report;

    protocol_sdk_enter_direct_mode();

    for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            protocol_sdk_set_key_led(row, col, rgb_convert_rgb565_to_argb(request->colors[row][col]));
        }
    }
}
PROTOCOL_REPORT_COMMAND(PROTOCOL_REPORT_CMD_SDK_SET_ALL_COLORS, protocol_cmd_sdk_set_all_colors);

/* Feature command 33: enters direct mode. arg is unused. */
static void protocol_cmd_sdk_init_colors(uint32_t arg, protocol_response_t *response)
{
    protocol_sdk_enter_direct_mode();
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_SDK_INIT_COLORS, protocol_cmd_sdk_init_colors);

/* ---- LED frame read-back ----------------------------------------------------------------------- */

/* Payload of feature command 74: RGB565 words, the key matrix row by row, then the light bar
 * (no words without one). */
#define PROTOCOL_LED_FRAME_KEYS_SIZE (RGB_KEY_COUNT * sizeof(uint16_t))
#define PROTOCOL_LED_FRAME_SIZE      (PROTOCOL_LED_FRAME_KEYS_SIZE + LIGHTBAR_LED_FRAME_SIZE)

_Static_assert(PROTOCOL_LED_FRAME_SIZE <= PROTOCOL_RESPONSE_PAYLOAD, "LED frame payload fits the response");

/* Stores a little-endian word at an arbitrary payload offset. */
static void protocol_put_u16(uint8_t *dst, uint16_t value)
{
    memcpy(dst, &value, sizeof(value));
}

/*
 * Feature command 74: dumps the colours of the main frame (before brightness and gamma) as
 * RGB565 words: every key position, then every light-bar LED. While the LEDs are switched off by
 * the idle timer the whole payload buffer is zeroed instead. arg is unused.
 */
static void protocol_cmd_get_led_frame(uint32_t arg, protocol_response_t *response)
{
    RgbColor last_key = { 0, 0, 0 };

    if ((g_rgb_idle_flags & RGB_IDLE_FLAG_OFF) != 0) {
        memset(response->payload, 0, sizeof(response->payload));
        response->length = PROTOCOL_LED_FRAME_SIZE;
        response->status = PROTOCOL_STATUS_OK;
        return;
    }

    for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            uint32_t key = row * RGB_MATRIX_COLS + col;

            protocol_put_u16(&response->payload[key * sizeof(uint16_t)],
                             rgb_frame_read_led_rgb565(&g_rgb_frame_main, g_rgb_key_to_led[key], &last_key));
        }
    }
    lightbar_dump_led_frame(&response->payload[PROTOCOL_LED_FRAME_KEYS_SIZE]);

    response->length = PROTOCOL_LED_FRAME_SIZE;
    response->status = PROTOCOL_STATUS_OK;
}
PROTOCOL_FEATURE_COMMAND(PROTOCOL_FEATURE_CMD_GET_LED_FRAME, protocol_cmd_get_led_frame);
