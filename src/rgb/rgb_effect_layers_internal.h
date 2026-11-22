/*
 * rgb_effect_layers_internal.h - interface between the effect layer dispatcher
 * (rgb_effect_layers.c) and the effect kinds that have a source file of their own
 * (rgb_effect_kind_*.c).
 *
 * Adding an effect: write one function with the rgb_effect_render_fn signature, declare it
 * here and add one `case` to the switch of rgb_render_effect_layers. The function receives
 * the frame holding the base colours plus every earlier layer and may overwrite LEDs
 * (rgb_frame_set_led) or dim what is there (rgb_frame_scale_led). Parameters live in
 * layer->cfg.effect (12 bytes), animation state in layer->state (rgb_effect_state_t). State
 * initialisation belongs into rgb_effect_reset_slot_state, the "needs a redraw"
 * poll into rgb_update_frame (see docs/extending-rgb.md).
 */
#ifndef RGB_RGB_EFFECT_LAYERS_INTERNAL_H_
#define RGB_RGB_EFFECT_LAYERS_INTERNAL_H_

#include <stdint.h>

#include "rgb/rgb_types.h"
#include "rgb/rgb_data.h"

/*
 * Uniform signature of an effect kind. elapsed_ms is the time since the previous frame
 * (0 on the frame that follows a state reset); it is a 32-bit value and each kind
 * truncates it to 16 bits wherever it uses a 16-bit timer.
 */
typedef void rgb_effect_render_fn(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms);

/* LED id of a key matrix position (RGB_LED_NONE where the layout has no LED). */
static inline uint8_t rgb_effect_key_led(uint32_t row, uint32_t col)
{
    return g_rgb_key_to_led[row * RGB_MATRIX_COLS + col];
}

rgb_effect_render_fn rgb_effect_kind_rainbow_wave;     /* kind 1,  rgb_effect_kind_rainbow_wave.c */
rgb_effect_render_fn rgb_effect_kind_key_ripple;       /* kind 5,  rgb_effect_kind_key_ripple.c */
rgb_effect_render_fn rgb_effect_kind_analog_spotlight; /* kind 8,  rgb_effect_kind_analog.c */
rgb_effect_render_fn rgb_effect_kind_analog_glow;      /* kind 9,  rgb_effect_kind_analog.c */
rgb_effect_render_fn rgb_effect_kind_key_test;         /* kind 11, rgb_effect_kind_key_test.c */
rgb_effect_render_fn rgb_effect_kind_simplex_noise;    /* kind 14, rgb_effect_kind_simplex_noise.c */
rgb_effect_render_fn rgb_effect_kind_polar_gradient;   /* kind 15, rgb_effect_kind_polar_gradient.c */

/* Kind 6, rgb_effect_kind_reactive_fade.c: own parameter list, wrapped by rgb_render_effect_layers. */
void rgb_render_reactive_fade_effect(rgb_frame_t *frame, const RgbEffectReactiveFade *params,
                                     rgb_effect_reactive_state_t *state, uint16_t elapsed_ms);

#endif /* RGB_RGB_EFFECT_LAYERS_INTERNAL_H_ */
