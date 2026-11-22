/*
 * lightbar.h - public header of feature `lightbar`: a row of LEDs that are not under keys,
 * with their own configuration (files `lb` per profile, `glb` and `glbm` for the keyboard,
 * `lbm` per profile), effects, brightness and host commands. A board lists the feature in
 * board.mk (together with feature `signals`, whose values the entries show) and gives its
 * constants in the light-bar block of board.h; without the feature the functions below do
 * nothing, LIGHTBAR_LED_COUNT is 0, the light-bar overlay has no parameters and the factory
 * content of the light-bar files is empty.
 *
 * Depends on rgb/rgb_types.h (frame and colour types, SignalRef, settings messages) and, with
 * the feature, on the light-bar constants of board.h.
 */
#ifndef RGB_LIGHTBAR_LIGHTBAR_H
#define RGB_LIGHTBAR_LIGHTBAR_H

#include <stdbool.h>
#include <stdint.h>

#include "rgb/rgb_types.h"

#if FEATURE_LIGHTBAR

/* Number of light-bar LEDs (positions 0..LIGHTBAR_LED_COUNT - 1). */
#define LIGHTBAR_LED_COUNT BOARD_LIGHTBAR_LED_COUNT

/* Bytes that lightbar_dump_led_frame writes: one RGB565 word per light-bar LED. */
#define LIGHTBAR_LED_FRAME_SIZE (LIGHTBAR_LED_COUNT * sizeof(uint16_t))

/* Parameters of overlay type 3: the colour of each light-bar position (alpha unused). */
typedef struct {
    rgb_argb_t colors[LIGHTBAR_LED_COUNT];
} lightbar_overlay_params_t;

/* Factory content of file `lb` of one built-in profile (data of the board's factory defaults,
 * profile/profile_defaults.h). */
typedef struct {
    bool has_file;       /* false: the profile gets no file `lb`, the loader defaults apply */
    uint8_t brightness;  /* brightness of the light bar of this profile, over the entries of `glb` */
} lightbar_profile_default_t;

/* Factory content of the root file `glb`: one profile indicator over the whole light bar. */
typedef struct {
    LightbarProfileBackground background_mode;
    uint8_t segment_count; /* segments of the indicator; 0: not given */
} lightbar_global_default_t;

/* True when the light bar has to be drawn again in this frame (since_render_ms: time since the
 * last render pass). */
bool lightbar_needs_render(uint16_t since_render_ms);

/* Render pass of the light bar: draws the light bar into frame, if the active profile has it
 * enabled, and on top of it the overlays of its class (lightbar_overlay_class) unless the host
 * drives the LEDs (RGB SDK mode). layer_colors and elapsed_ms as for rgb_render_overlays. */
void lightbar_render_pass(rgb_frame_t *frame, const uint16_t *layer_colors, uint16_t elapsed_ms);

/* Draws the light bar on the next frame regardless of what changed. */
void lightbar_request_redraw(void);

/* Flush step: the light-bar channels of a frame page are written to out with the light-bar
 * brightness (times fade_level / 255) instead of the key brightness. */
void lightbar_apply_page_brightness(const rgb_frame_page_t *page, rgb_frame_page_t *out, uint32_t fade_level);

/* Brightness of the light-bar LEDs, 0 = off. */
uint8_t lightbar_brightness(void);

/* The key brightness changed to key_brightness: a light bar without a brightness of its own
 * follows it. */
void lightbar_apply_brightness_change(uint32_t key_brightness);

/* The scan mode changed between off and on, which changes what the entries may show: the
 * entries are bound again. */
void lightbar_apply_scan_mode_change(void);

/* Redraw class (RGB_OVERLAY_CLASS_*) of the light-bar overlay type. The overlays of this class
 * are drawn on top of the light bar (lightbar_render_pass), the others below it. */
uint8_t lightbar_overlay_class(void);

/* Draws the colours of a light-bar overlay, and clears the light bar when such an overlay
 * ends. */
void lightbar_draw_overlay(rgb_frame_t *frame, const lightbar_overlay_params_t *params);
void lightbar_clear_overlay(rgb_frame_t *frame);

/* LED frame dump (feature command 74): writes the colours of the light-bar LEDs in the main
 * frame to out as LIGHTBAR_LED_FRAME_SIZE bytes of RGB565 words, in light-bar order. */
void lightbar_dump_led_frame(uint8_t *out);

/* Signal subscriber of kind RGB_SUBSCRIBER_KIND_LIGHTBAR: delivers a changed signal value to
 * the light-bar entry named by `subscriber`. */
void rgb_lightbar_notify_signal(uint32_t subscriber, const SignalRef *signal_ref, float value);

/* First boot, file `lb` of built-in profile profile_index is missing: writes it with the
 * factory content `defaults` if that gives one, and then sets *created (also when the storage
 * is not mounted and nothing is written). */
void lightbar_write_default_profile_file(uint32_t profile_index, const lightbar_profile_default_t *defaults,
                                         bool *created);

/* First boot, the root file `glb` is missing: writes it with the factory content `defaults`. */
void lightbar_write_default_global_file(const lightbar_global_default_t *defaults);

#else

#define LIGHTBAR_LED_COUNT 0
#define LIGHTBAR_LED_FRAME_SIZE 0u

/* No light bar: overlay type 3 has no parameters, so the overlay record keeps only the key and
 * fade members. */
typedef struct {
} lightbar_overlay_params_t;

/* No light bar: the factory defaults hold no light-bar content. */
typedef struct {
} lightbar_profile_default_t;

typedef struct {
} lightbar_global_default_t;

static inline bool lightbar_needs_render(uint16_t since_render_ms)
{
    return false;
}

static inline void lightbar_render_pass(rgb_frame_t *frame, const uint16_t *layer_colors, uint16_t elapsed_ms)
{
}

static inline void lightbar_request_redraw(void)
{
}

static inline void lightbar_apply_page_brightness(const rgb_frame_page_t *page, rgb_frame_page_t *out,
                                                  uint32_t fade_level)
{
}

static inline uint8_t lightbar_brightness(void)
{
    return 0;
}

static inline void lightbar_apply_brightness_change(uint32_t key_brightness)
{
}

static inline void lightbar_apply_scan_mode_change(void)
{
}

static inline uint8_t lightbar_overlay_class(void)
{
    return 0;
}

static inline void lightbar_draw_overlay(rgb_frame_t *frame, const lightbar_overlay_params_t *params)
{
}

static inline void lightbar_clear_overlay(rgb_frame_t *frame)
{
}

static inline void lightbar_dump_led_frame(uint8_t *out)
{
}

static inline void rgb_lightbar_notify_signal(uint32_t subscriber, const SignalRef *signal_ref, float value)
{
}

static inline void lightbar_write_default_profile_file(uint32_t profile_index,
                                                       const lightbar_profile_default_t *defaults, bool *created)
{
}

static inline void lightbar_write_default_global_file(const lightbar_global_default_t *defaults)
{
}

#endif /* FEATURE_LIGHTBAR */

#endif /* RGB_LIGHTBAR_LIGHTBAR_H */
