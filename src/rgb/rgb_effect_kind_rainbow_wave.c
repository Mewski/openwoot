/*
 * rgb_effect_kind_rainbow_wave.c - effect kind 1: hue gradient moving over the matrix columns.
 * Every matrix position is overwritten, one colour per column.
 */
#include "rgb/rgb_effect_layers_internal.h"
#include "rgb/rgb_frame.h"

#define HUE_SECTOR_WIDTH 43 /* 256 / 6 */

/*
 * Integer HSV to RGB conversion with S = V = 255 (six sectors of 43 hue steps). The same
 * conversion is used by rgb_effect_render_hue_cycle.
 */
static void rainbow_hue_to_rgb(uint8_t hue, uint8_t *r, uint8_t *g, uint8_t *b)
{
    uint32_t sector  = hue / HUE_SECTOR_WIDTH;
    int32_t  rem     = (uint16_t)((hue % HUE_SECTOR_WIDTH) * 6);
    uint8_t  falling = (uint8_t)((255 * (255 - ((255 * rem) >> 8))) >> 8);
    uint8_t  rising  = (uint8_t)((255 * (255 - ((255 * (255 - rem)) >> 8))) >> 8);

    switch (sector) {
    case 0:
        *r = 255;
        *g = rising;
        *b = 0;
        break;
    case 1:
        *r = falling;
        *g = 255;
        *b = 0;
        break;
    case 2:
        *r = 0;
        *g = 255;
        *b = rising;
        break;
    case 3:
        *r = 0;
        *g = falling;
        *b = 255;
        break;
    case 4:
        *r = rising;
        *g = 0;
        *b = 255;
        break;
    default:
        *r = 255;
        *g = 0;
        *b = falling;
        break;
    }
}

/*
 * Kind 1 renderer: advances the 16-bit phase (one turn per period_250ms * 250 ms, direction
 * from the parameters) and paints each column in the hue of its position in the wave.
 */
void rgb_effect_kind_rainbow_wave(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    const RgbEffectRainbowWave *params = &layer->cfg.effect.rainbow_wave;
    rgb_effect_hue_state_t     *state  = &layer->state.hue;

    /*
     * KNOWN ISSUE: a period of 0 divides by zero. The Cortex-M4 UDIV returns 0 (the
     * divide-by-zero trap is not enabled), so the phase step is 0 and the animation freezes.
     * Matches stock firmware 2.14.1.
     */
    uint16_t step = (uint16_t)((0x10000u / (250u * params->period_250ms)) * elapsed_ms);
    uint16_t phase;
    int32_t  first;
    int32_t  second;

    if (params->direction != 0) {
        phase  = (uint16_t)(state->phase + step);
        first  = state->hue_a;
        second = state->hue_b;
    } else {
        phase  = (uint16_t)(state->phase - step);
        first  = state->hue_b;
        second = state->hue_a;
    }
    state->phase = phase;

    for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
        uint8_t hue;
        uint8_t r;
        uint8_t g;
        uint8_t b;

        if (params->hue_preset == RgbHuePreset_RGB_HUE_PRESET_FULL_CIRCLE) {
            /* Sawtooth: one period across the columns. */
            uint8_t pos = (uint8_t)((phase >> 8) + 255 * col / RGB_MATRIX_COLS);

            hue = (uint8_t)(((first << 8) + pos * (second - first)) >> 8);
        } else {
            /* Triangle: half a period across the columns, the hue range is run back and forth. */
            uint8_t pos = (uint8_t)((phase >> 8) + 255 * col / (2 * RGB_MATRIX_COLS));

            if (pos < 128) {
                hue = (uint8_t)(((first << 8) + 2 * pos * (second - first) + 128) >> 8);
            } else {
                hue = (uint8_t)(((second << 8) + 2 * (pos - 128) * (first - second) + 128) >> 8);
            }
        }

        rainbow_hue_to_rgb(hue, &r, &g, &b);
        for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
            rgb_frame_set_led(frame, rgb_effect_key_led(row, col), r, g, b);
        }
    }
}
