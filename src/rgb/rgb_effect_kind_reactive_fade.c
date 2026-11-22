/*
 * rgb_effect_kind_reactive_fade.c - effect layer kind 6 (reactive fade) of the key effect
 * renderer rgb_render_effect_layers, which wraps it.
 *
 * Every key has one cell byte that rgb_effects_handle_key_event arms on a key press and
 * that this function counts down and draws. Three modes, selected by params->flags:
 *   plain        (no bit)  the colour already in the frame is scaled by the level
 *   palette      (bit 0)   the key shows a random palette colour at the level
 *   fixed colour (bit 1)   the key shows params->color, then cross-fades back to the frame
 */
#include "rgb/rgb_effect_layers_internal.h"
#include "rgb/rgb_frame.h"
#include "tables/rgb_color_tables.h"

/* RgbEffectReactiveFade.flags */
#define REACTIVE_FLAG_PALETTE      0x01
#define REACTIVE_FLAG_FIXED_COLOR  0x02 /* ignored with REACTIVE_FLAG_PALETTE */
#define REACTIVE_FLAG_INVERT       0x04

#define REACTIVE_STEP_MS_MAX       40
#define REACTIVE_PALETTE_STEP_MUL  8

/* plain / fixed-colour cells: RGB_REACTIVE_CELL_PRESSED (251) just pressed, 250..1 counting down */
#define REACTIVE_LEVEL_MAX         250
/* fixed-colour mode: above this level (of 255) the colour only dims, below it cross-fades */
#define REACTIVE_CROSSFADE_LEVEL   100

/* palette cells: bits 7:3 level (31 just pressed, 30..1), bits 2:0 palette index 1..7 */
#define REACTIVE_PALETTE_COLOR_MASK  0x07
#define REACTIVE_PALETTE_LEVEL_MASK  0xF8
#define REACTIVE_PALETTE_LEVEL_SHIFT 3
#define REACTIVE_PALETTE_LEVEL_MAX   30

enum reactive_mode {
    REACTIVE_MODE_PLAIN = 0,
    REACTIVE_MODE_PALETTE = 1,
    REACTIVE_MODE_FIXED_COLOR = 2
};

/* Brightness factor between base_level (level 0) and 255 (level 255), 8.8 fixed point. */
static inline uint8_t reactive_scale(uint8_t base_level, uint8_t level)
{
    return (uint8_t)((uint16_t)((base_level << 8) + (255 - base_level) * level) >> 8);
}

/*
 * Kind 6 renderer: counts every key cell down by the steps that fit into elapsed_ms
 * (step_ms 1..40, times 8 in palette mode), draws the keys in the mode selected by
 * params->flags and sets state->active while any key is still animating.
 */
void rgb_render_reactive_fade_effect(rgb_frame_t *frame, const RgbEffectReactiveFade *params, rgb_effect_reactive_state_t *state, uint16_t elapsed_ms)
{
    uint8_t flags = params->flags;
    bool invert = (flags & REACTIVE_FLAG_INVERT) != 0;
    uint8_t base_level = params->base_level;
    enum reactive_mode mode;
    uint32_t step_ms;
    uint32_t total_ms;
    uint32_t steps;

    step_ms = params->step_ms;
    if (step_ms == 0) {
        step_ms = 1;
    }
    if (step_ms >= REACTIVE_STEP_MS_MAX) {
        step_ms = REACTIVE_STEP_MS_MAX;
    }
    if ((flags & REACTIVE_FLAG_PALETTE) != 0) {
        mode = REACTIVE_MODE_PALETTE;
        step_ms *= REACTIVE_PALETTE_STEP_MUL;
    } else if ((flags & REACTIVE_FLAG_FIXED_COLOR) != 0) {
        mode = REACTIVE_MODE_FIXED_COLOR;
    } else {
        mode = REACTIVE_MODE_PLAIN;
    }

    /* elapsed time -> number of countdown steps, remainder kept for the next frame */
    total_ms = (uint16_t)(elapsed_ms + state->timer_ms);
    state->active = 0;
    if (total_ms >= step_ms) {
        steps = (uint8_t)(total_ms / step_ms);
        state->timer_ms = (uint16_t)(total_ms - steps * step_ms);
    } else {
        state->timer_ms = (uint16_t)total_ms;
        steps = 0;
    }

    for (uint32_t row = 0; row != RGB_MATRIX_ROWS; row++) {
        for (uint32_t col = 0; col != RGB_MATRIX_COLS; col++) {
            uint8_t *cell = &state->cells[row][col];
            uint32_t value = *cell;

            if (mode == REACTIVE_MODE_PALETTE) {
                uint8_t level = 0;
                uint8_t palette_index;
                uint8_t r = 0;
                uint8_t g = 0;
                uint8_t b = 0;

                if ((value & REACTIVE_PALETTE_LEVEL_MASK) != 0) {
                    uint32_t count = value >> REACTIVE_PALETTE_LEVEL_SHIFT;

                    if (steps <= count) {
                        if ((value & REACTIVE_PALETTE_LEVEL_MASK) == REACTIVE_PALETTE_LEVEL_MASK) {
                            count = (count + 31) & 0x1F; /* just pressed (31) -> 30 */
                        } else {
                            count = (count - steps) & 0x1F;
                        }
                        *cell = (uint8_t)((value & REACTIVE_PALETTE_COLOR_MASK) | (count << REACTIVE_PALETTE_LEVEL_SHIFT));
                        level = (uint8_t)(255 * count / REACTIVE_PALETTE_LEVEL_MAX);
                    } else {
                        *cell = (uint8_t)(value & REACTIVE_PALETTE_COLOR_MASK);
                    }
                    if ((*cell & REACTIVE_PALETTE_LEVEL_MASK) != 0) {
                        state->active = 1;
                    }
                }
                if (invert) {
                    level = (uint8_t)~level;
                }

                palette_index = (uint8_t)((*cell & REACTIVE_PALETTE_COLOR_MASK) - 1);
                if (palette_index < RGB_PALETTE_COLOR_COUNT) {
                    r = k_rgb_palette_red[palette_index];
                    g = k_rgb_palette_green[palette_index];
                    b = k_rgb_palette_blue[palette_index];
                }
                rgb_frame_set_key_scaled(frame, (uint8_t)row, (uint8_t)col, r, g, b, reactive_scale(base_level, level));
                continue;
            }

            /* plain and fixed-colour mode: count the cell down */
            uint32_t countdown = 0;

            if (value != 0) {
                if (steps <= value) {
                    if (value == RGB_REACTIVE_CELL_PRESSED) {
                        countdown = REACTIVE_LEVEL_MAX;
                    } else {
                        countdown = (uint8_t)(value - steps);
                    }
                    *cell = (uint8_t)countdown;
                } else {
                    *cell = 0;
                }
            }
            if (countdown != 0) {
                state->active = 1;
            }

            if (mode == REACTIVE_MODE_FIXED_COLOR) {
                uint32_t color;
                uint32_t level;
                uint8_t color_b, color_g, color_r;

                if (countdown == 0) {
                    /* idle key: frame colour at the base level */
                    rgb_frame_scale_led(frame, g_rgb_key_to_led[RGB_MATRIX_COLS * row + col], base_level);
                    continue;
                }

                color = rgb_unpack_color(params->color);
                color_b = (uint8_t)color;
                color_g = (uint8_t)(color >> 8);
                color_r = (uint8_t)(color >> 16);
                level = (uint8_t)(255 * countdown / REACTIVE_LEVEL_MAX);

                if (level < REACTIVE_CROSSFADE_LEVEL) {
                    /* last part: cross-fade from the fixed colour back to the frame colour.
                     * The invert flag is not applied in this mode. */
                    uint8_t page_r, page_g, page_b;
                    uint8_t *ch_r, *ch_g, *ch_b;

                    if (rgb_frame_locate_led(frame, g_rgb_key_to_led[RGB_MATRIX_COLS * row + col],
                                             &page_r, &page_g, &page_b, &ch_r, &ch_g, &ch_b) != 0) {
                        int16_t weight = (int16_t)(255u * (uint16_t)(REACTIVE_CROSSFADE_LEVEL - level) / 100);

                        rgb_frame_set_key_scaled(
                            frame, (uint8_t)row, (uint8_t)col,
                            (uint8_t)((uint16_t)((*ch_r - color_r) * weight + (color_r << 8)) >> 8),
                            (uint8_t)((uint16_t)((*ch_g - color_g) * weight + (color_g << 8)) >> 8),
                            (uint8_t)((uint16_t)((*ch_b - color_b) * weight + (color_b << 8)) >> 8),
                            base_level);
                    }
                } else {
                    /* first part: fixed colour, brightness falling towards the base level */
                    uint8_t dim = (uint8_t)(255 * (level - REACTIVE_CROSSFADE_LEVEL) / (255 - REACTIVE_CROSSFADE_LEVEL));

                    rgb_frame_set_key_scaled(frame, (uint8_t)row, (uint8_t)col, color_r, color_g, color_b,
                                             reactive_scale(base_level, dim));
                }
                continue;
            }

            /* plain mode: scale the colour in the frame */
            uint8_t level = (uint8_t)(255 * countdown / REACTIVE_LEVEL_MAX);

            if (invert) {
                level = (uint8_t)~level;
            }
            rgb_frame_scale_led(frame, g_rgb_key_to_led[RGB_MATRIX_COLS * row + col], reactive_scale(base_level, level));
        }
    }
}
