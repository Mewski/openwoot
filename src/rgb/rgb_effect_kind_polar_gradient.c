/*
 * rgb_effect_kind_polar_gradient.c - effect kind 15: two-colour angular gradient rotating
 * around the keyboard centre (row 3.0, column 8.5). Needs g_rgb_key_angle_table and
 * g_rgb_key_distance_table, built once by rgb_init_polar_tables. Every matrix
 * position is overwritten.
 */
#include <math.h>

#include "rgb/rgb_effect_layers_internal.h"
#include "rgb/rgb_frame.h"

#define POLAR_PHASE_MODULUS 65535

/*
 * Kind 15 renderer: advances the rotation phase by elapsed_ms * speed and colours every key
 * by its angle around the centre plus twist * distance.
 */
void rgb_effect_kind_polar_gradient(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    const RgbEffectPolarGradient *params = &layer->cfg.effect.polar_gradient;
    rgb_effect_polar_state_t     *state  = &layer->state.polar;

    /*
     * KNOWN ISSUE: with a negative speed the sum goes negative, the C remainder stays negative
     * and its truncation to 16 bits wraps at 65536 instead of 65535 (one phase count, not
     * visible). Matches stock firmware 2.14.1.
     */
    int32_t phase_sum = (int32_t)(state->phase + elapsed_ms * (uint32_t)(int32_t)params->speed);

    state->phase = (uint16_t)(phase_sum % POLAR_PHASE_MODULUS);

    uint32_t color_a = rgb_unpack_color(params->color_a);
    uint32_t color_b = rgb_unpack_color(params->color_b);
    float    phase   = (float)state->phase / 65535.0f;
    float    twist   = (float)params->twist / 127.0f * 0.1f; /* phase offset per unit of distance: spiral */

    for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            /* Both multiply-adds are fused (fmaf), so the intermediate product is not rounded. */
            float    angle    = (float)g_rgb_key_angle_table[row][col] / 255.0f - 0.5f + phase;
            float    distance = (float)g_rgb_key_distance_table[row][col] * 0.0625f;
            float    turns    = fmaf(twist, distance, angle);
            uint8_t  position = (uint8_t)(uint32_t)fmaf(turns - floorf(turns), 255.0f, 0.5f);
            uint32_t color;

            /* Around the circle: colour A at 0, colour B at 128, back to colour A at 255. */
            if (position == 0 || position == 255) {
                color = color_a;
            } else if (position == 128) {
                color = color_b;
            } else if (position < 128) {
                color = rgb_blend_argb(color_a, color_b, (uint8_t)(2 * position));
            } else {
                color = rgb_blend_argb(color_a, color_b, (uint8_t)~(2 * (position - 128)));
            }
            rgb_frame_set_led(frame, rgb_effect_key_led(row, col), (uint8_t)(color >> 16), (uint8_t)(color >> 8),
                              (uint8_t)color);
        }
    }
}
