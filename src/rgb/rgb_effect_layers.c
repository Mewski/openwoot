/*
 * rgb_effect_layers.c - per-frame renderer of the key matrix LEDs: base colour map plus up to
 * six effect layers drawn on top of each other in list order.
 *
 * rgb_render_effect_layers is a small dispatcher; every effect kind is one function with the
 * rgb_effect_render_fn signature (rgb_effect_layers_internal.h). Kinds 1, 5, 8, 9, 11, 14, 15
 * live in rgb_effect_kind_*.c, kind 6 in rgb_effect_kind_reactive_fade.c with parameters of its
 * own; kinds 2, 3, 4, 6, 7, 10 are wrapped below.
 */
#include "rgb/rgb_effect_layers.h"

#include "proto/settings.pb.h"
#include "rgb/rgb_effect_layers_internal.h"
#include "rgb/rgb_effects.h"
#include "rgb/rgb_frame.h"

/*
 * Re-initialises the animation state of every layer. Only reached through
 * g_rgb_effect_reset_request, which nothing in this firmware sets.
 */
static void rgb_effect_layers_reset_all(void)
{
    for (uint8_t i = 0; i < g_rgb_effect_layer_count; i++) {
        rgb_effect_reset_slot_state(&g_rgb_effect_layers[i]);
    }
}

/*
 * Paints the RGB565 base colour of every matrix position into the frame (column by column).
 * Same expansion as rgb_convert_rgb565_to_argb: the low bits of a non-zero channel
 * are filled with ones.
 */
static void rgb_effect_layers_paint_base_colors(rgb_frame_t *frame)
{
    for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
        for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
            uint16_t rgb565 = g_rgb_base_colors[row][col];
            uint8_t  r = (uint8_t)((rgb565 >> 8) & 0xF8);
            uint8_t  g = (uint8_t)((rgb565 >> 3) & 0xFC);
            uint8_t  b = (uint8_t)(rgb565 << 3);

            if (r != 0) {
                r += 7;
            }
            if (g != 0) {
                g += 3;
            }
            if (b != 0) {
                b += 7;
            }
            rgb_frame_set_led(frame, rgb_effect_key_led(row, col), r, g, b);
        }
    }
}

/* ---- kinds rendered by functions of other units --------------------------------------------- */

/* Kind 2: bright window sweeping back and forth. */
static void rgb_effect_kind_scanner(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    rgb_effect_render_scanner(frame, &layer->cfg.effect.scanner, &layer->state.scanner, (uint16_t)elapsed_ms);
}

/* Kind 3: all key LEDs fade up and down. */
static void rgb_effect_kind_breathing(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    rgb_effect_render_breathing(frame, layer->cfg.effect.breathing.step_ms, layer->cfg.effect.breathing.min_level,
                                &layer->state.breathing, (uint16_t)elapsed_ms);
}

/* Kind 4: all keys in one colour that cycles through a hue range. */
static void rgb_effect_kind_hue_cycle(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    rgb_effect_render_hue_cycle(frame, layer->cfg.effect.hue_cycle.hue_preset, layer->cfg.effect.hue_cycle.period_250ms,
                                &layer->state.hue, (uint16_t)elapsed_ms);
}

/* Kind 6: pressed keys light up and fade out. */
static void rgb_effect_kind_reactive_fade(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    rgb_render_reactive_fade_effect(frame, &layer->cfg.effect.reactive_fade, &layer->state.reactive,
                                    (uint16_t)elapsed_ms);
}

/* Kind 7: bar graph of the deepest pressed key on the digit row. */
static void rgb_effect_kind_level_meter(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    rgb_effect_render_level_meter(frame, layer->cfg.effect.level_meter.base_level, &layer->state.level_meter);
}

/* Kind 10: all keys in the colour given by the depth of three keys. */
static void rgb_effect_kind_analog_rgb_fill(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    rgb_frame_fill_keys(frame, layer->state.fill.red, layer->state.fill.green, layer->state.fill.blue);
}

/* ---- dispatcher ----------------------------------------------------------------------------- */

/*
 * Draws the base colour map and all effect layers of the active profile into frame (the
 * caller rgb_update_frame passes a private frame and the time since the previous
 * frame, then adds overlays and the light bar). Kinds 12, 13 and unknown kinds draw nothing.
 */
void rgb_render_effect_layers(rgb_frame_t *frame, uint32_t elapsed_ms)
{
    g_rgb_effect_redraw_request = 0;
    if (g_rgb_effect_reset_request != 0) {
        g_rgb_effect_reset_request = 0;
        rgb_effect_layers_reset_all();
        elapsed_ms = 0;
    }

    rgb_effect_layers_paint_base_colors(frame);

    /* The layer count is read again after every layer. */
    for (uint8_t i = 0; i < g_rgb_effect_layer_count; i++) {
        rgb_effect_layer_t *layer = &g_rgb_effect_layers[i];

        switch (layer->cfg.which_effect) {
        case RgbEffectLayer_rainbow_wave_tag:
            rgb_effect_kind_rainbow_wave(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_scanner_tag:
            rgb_effect_kind_scanner(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_breathing_tag:
            rgb_effect_kind_breathing(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_hue_cycle_tag:
            rgb_effect_kind_hue_cycle(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_key_ripple_tag:
            rgb_effect_kind_key_ripple(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_reactive_fade_tag:
            rgb_effect_kind_reactive_fade(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_level_meter_tag:
            rgb_effect_kind_level_meter(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_analog_spotlight_tag:
            rgb_effect_kind_analog_spotlight(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_analog_glow_tag:
            rgb_effect_kind_analog_glow(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_analog_rgb_fill_tag:
            rgb_effect_kind_analog_rgb_fill(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_key_test_tag:
            rgb_effect_kind_key_test(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_simplex_noise_tag:
            rgb_effect_kind_simplex_noise(frame, layer, elapsed_ms);
            break;
        case RgbEffectLayer_polar_gradient_tag:
            rgb_effect_kind_polar_gradient(frame, layer, elapsed_ms);
            break;
        default: /* RgbEffectLayer_f12_tag, RgbEffectLayer_f13_tag and anything else: not drawn */
            break;
        }
    }
}
