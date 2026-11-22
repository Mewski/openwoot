/*
 * rgb_effects_render.c - the effect renderers with parameters passed by value:
 * hue cycle (kind 4), scanner (kind 2), level meter (kind 7) and breathing (kind 3).
 *
 * All four are called once per frame and layer by rgb_render_effect_layers. The hue
 * cycle paints the key LEDs; the other three only dim what is already in the frame
 * (rgb_frame_scale_led), so they work as masks on top of the base colours and lower layers.
 */
#include "rgb/rgb_effects.h"

#include "rgb/rgb_frame.h"

/* ------------------------------------------------------------------------------------------
 * Kind 4: hue cycle
 * ------------------------------------------------------------------------------------------ */

#define RGB_HUE_SEXTANT_SIZE 43 /* 256 hues / 6 sextants */

/*
 * Kind 4: paints all key LEDs in one hue that moves through the range [hue_a, hue_b] of the
 * state (palette 0: sawtooth over the full circle, else triangle over the preset range).
 */
void rgb_effect_render_hue_cycle(rgb_frame_t *frame, uint32_t palette, uint32_t period,
                                 rgb_effect_hue_state_t *state, uint16_t elapsed_ms)
{
    /* One cycle takes 250 * period ms.
     * KNOWN ISSUE: a period of 0 is not guarded. The Cortex-M4 UDIV returns 0 for
     * a division by zero (the divide-by-zero trap is not enabled), so the phase simply stops.
     * Matches stock firmware 2.14.1. */
    uint16_t phase = (uint16_t)(0x10000u / (250u * period) * elapsed_ms + state->phase);
    uint32_t pos = phase >> 8; /* 0..255 position inside the cycle */
    uint8_t hue;

    state->phase = phase;

    if (palette == 0) {
        /* Full circle: sawtooth from hue_a to hue_b. */
        hue = (uint8_t)(((state->hue_a << 8) + (state->hue_b - state->hue_a) * (int32_t)pos) >> 8);
    } else {
        /* Preset range: triangle hue_a -> hue_b -> hue_a, rounded. */
        int32_t hue_a = state->hue_a;
        int32_t hue_b = state->hue_b;

        if ((pos & 0x80) == 0) {
            hue = (uint8_t)((uint16_t)((hue_a << 8) + 2 * (int32_t)pos * (hue_b - hue_a) + 0x80) >> 8);
        } else {
            hue = (uint8_t)((uint16_t)((hue_b << 8) + (hue_a - hue_b) * 2 * ((int32_t)pos - 0x80) + 0x80) >> 8);
        }
    }

    /* Hue to RGB at full saturation and value. */
    uint32_t sextant = hue / RGB_HUE_SEXTANT_SIZE;
    int32_t fraction = (uint16_t)(6 * (hue % RGB_HUE_SEXTANT_SIZE)); /* 0..252 */
    uint8_t falling = (uint8_t)((255 * (255 - ((255 * fraction) >> 8))) >> 8);
    uint8_t rising = (uint8_t)((255 * (255 - ((255 * (255 - fraction)) >> 8))) >> 8);
    uint8_t r, g, b;

    switch (sextant) {
    case 0:
        r = 255;
        g = rising;
        b = 0;
        break;
    case 1:
        r = falling;
        g = 255;
        b = 0;
        break;
    case 2:
        r = 0;
        g = 255;
        b = rising;
        break;
    case 3:
        r = 0;
        g = falling;
        b = 255;
        break;
    case 4:
        r = rising;
        g = 0;
        b = 255;
        break;
    default:
        r = 255;
        g = 0;
        b = falling;
        break;
    }

    for (uint32_t row = 0; row != RGB_MATRIX_ROWS; ++row) {
        for (uint32_t col = 0; col != RGB_MATRIX_COLS; ++col) {
            rgb_frame_set_led(frame, g_rgb_key_to_led[row * RGB_MATRIX_COLS + col], r, g, b);
        }
    }
}

/* ------------------------------------------------------------------------------------------
 * Kind 2: scanner
 * ------------------------------------------------------------------------------------------ */

#define RGB_SCANNER_STEPS_PER_SECOND_MAX 60

/*
 * Kind 2: a window of `width` columns (or rows) sweeps back and forth at steps_per_second
 * (1..60); everything outside the window is dimmed to outside_level.
 */
void rgb_effect_render_scanner(rgb_frame_t *frame, const RgbEffectScanner *params,
                               rgb_effect_scanner_state_t *state, uint16_t elapsed_ms)
{
    uint32_t timer_ms = (uint16_t)(elapsed_ms + state->timer_ms);
    uint32_t steps_per_second = params->steps_per_second;
    uint32_t outside_level = params->outside_level;
    uint32_t step_ms;
    uint32_t phase;

    state->timer_ms = (uint16_t)timer_ms;

    if (steps_per_second == 0) {
        steps_per_second = 1;
    }
    if (steps_per_second >= RGB_SCANNER_STEPS_PER_SECOND_MAX) {
        steps_per_second = RGB_SCANNER_STEPS_PER_SECOND_MAX;
    }
    step_ms = 1000u / steps_per_second;
    phase = state->phase;

    /* Move the window by one column / row per elapsed step. */
    while (step_ms < timer_ms) {
        int32_t window_end;

        timer_ms = (uint16_t)(timer_ms - step_ms);
        state->timer_ms = (uint16_t)timer_ms;

        switch (phase) {
        case RgbScannerDirection_RGB_SCANNER_DIRECTION_LEFT:
            window_end = (int8_t)(state->window_end - 1);
            state->window_end = (int8_t)window_end;
            state->window_start = (int8_t)(state->window_start - 1);
            if (window_end <= (int32_t)(((uint32_t)params->width + 1) >> 1)) {
                phase = RgbScannerDirection_RGB_SCANNER_DIRECTION_RIGHT;
                state->phase = (uint8_t)phase;
            }
            break;
        case RgbScannerDirection_RGB_SCANNER_DIRECTION_RIGHT:
            window_end = (int8_t)(state->window_end + 1);
            state->window_end = (int8_t)window_end;
            state->window_start = (int8_t)(state->window_start + 1);
            if ((params->width >> 1) + (RGB_MATRIX_COLS - 1) < window_end) {
                phase = RgbScannerDirection_RGB_SCANNER_DIRECTION_LEFT;
                state->phase = (uint8_t)phase;
            }
            break;
        case RgbScannerDirection_RGB_SCANNER_DIRECTION_UP:
            window_end = (int8_t)(state->window_end - 1);
            state->window_end = (int8_t)window_end;
            state->window_start = (int8_t)(state->window_start - 1);
            if (window_end <= (int32_t)(((uint32_t)params->width + 1) >> 1) + RGB_FIRST_KEY_ROW) {
                phase = RgbScannerDirection_RGB_SCANNER_DIRECTION_DOWN;
                state->phase = (uint8_t)phase;
            }
            break;
        case RgbScannerDirection_RGB_SCANNER_DIRECTION_DOWN:
            window_end = (int8_t)(state->window_end + 1);
            state->window_end = (int8_t)window_end;
            state->window_start = (int8_t)(state->window_start + 1);
            if ((params->width >> 1) + (RGB_MATRIX_ROWS - 1) < window_end) {
                phase = RgbScannerDirection_RGB_SCANNER_DIRECTION_UP;
                state->phase = (uint8_t)phase;
            }
            break;
        default:
            /* Unknown phase: the time is consumed, nothing moves. */
            break;
        }
    }

    if (phase <= RgbScannerDirection_RGB_SCANNER_DIRECTION_RIGHT) {
        /* Window of columns: dim every column outside [window_start, window_end). */
        for (int32_t col = 0; col != RGB_MATRIX_COLS; ++col) {
            if (col >= state->window_start && col < state->window_end) {
                continue;
            }
            for (int32_t row = 0; row != RGB_MATRIX_ROWS; ++row) {
                rgb_frame_scale_led(frame, g_rgb_key_to_led[row * RGB_MATRIX_COLS + col], outside_level);
            }
        }
    } else if (phase <= RgbScannerDirection_RGB_SCANNER_DIRECTION_DOWN) {
        /* Window of rows: dim every key row outside [window_start, window_end). */
        for (int32_t row = RGB_FIRST_KEY_ROW; row != RGB_MATRIX_ROWS; ++row) {
            if (state->window_start > row || state->window_end <= row) {
                for (int32_t col = 0; col != RGB_MATRIX_COLS; ++col) {
                    rgb_frame_scale_led(frame, g_rgb_key_to_led[row * RGB_MATRIX_COLS + col], outside_level);
                }
            }
        }
    }
}

/* ------------------------------------------------------------------------------------------
 * Kind 7: level meter
 * ------------------------------------------------------------------------------------------ */

#define RGB_LEVEL_METER_ROW       1    /* the bar lives on row 1 ... */
#define RGB_LEVEL_METER_FIRST_COL 1    /* ... from column 1 on, one key per segment */
#define RGB_LEVEL_METER_FULL      4095 /* key travel range */

/*
 * Kind 7: bar graph of state->level (0..4095) on RGB_LEVEL_METER_SEGMENTS keys of row 1 from
 * column 1; keys above the level are dimmed to min_scale. Clears the dirty flag.
 */
void rgb_effect_render_level_meter(rgb_frame_t *frame, uint32_t min_scale,
                                   rgb_effect_level_meter_state_t *state)
{
    /*
     * The segment bounds are k * 4095 / RGB_LEVEL_METER_SEGMENTS. Key k keeps its colour when
     * the level is at or above the upper bound of its segment, is dimmed to min_scale below the
     * lower bound and proportionally inside the segment.
     */
    for (uint32_t segment = 0; segment != RGB_LEVEL_METER_SEGMENTS; ++segment) {
        uint32_t lower = segment * RGB_LEVEL_METER_FULL / RGB_LEVEL_METER_SEGMENTS;
        uint32_t upper = (segment + 1) * RGB_LEVEL_METER_FULL / RGB_LEVEL_METER_SEGMENTS;
        uint32_t level = state->level; /* re-read for every key */
        uint32_t led_id = g_rgb_key_to_led[RGB_LEVEL_METER_ROW * RGB_MATRIX_COLS + RGB_LEVEL_METER_FIRST_COL + segment];

        if (level < lower) {
            rgb_frame_scale_led(frame, led_id, min_scale);
        } else if (level < upper) {
            uint32_t fraction = 255 * (level - lower) / (upper - lower);

            rgb_frame_scale_led(frame, led_id,
                                (uint8_t)(((min_scale << 8) + fraction * (255 - (uint16_t)min_scale)) >> 8));
        }
    }

    state->dirty = 0;
}

/* ------------------------------------------------------------------------------------------
 * Kind 3: breathing
 * ------------------------------------------------------------------------------------------ */

#define RGB_BREATHING_STEP_MS_MAX 40
#define RGB_BREATHING_LEVEL_MAX   250
#define RGB_BREATHING_LEVEL_STEP  2

/*
 * Kind 3: dims all key LEDs with a level that rises to 250 and falls back to 0 in steps of 2,
 * one step per step_ms (1..40); the scale runs from min_scale to 255.
 */
void rgb_effect_render_breathing(rgb_frame_t *frame, uint32_t step_ms, uint32_t min_scale,
                                 rgb_effect_breathing_state_t *state, uint16_t elapsed_ms)
{
    uint32_t timer_ms;
    uint32_t level;
    uint32_t scale;

    if (step_ms == 0) {
        step_ms = 1;
    }
    if (step_ms >= RGB_BREATHING_STEP_MS_MAX) {
        step_ms = RGB_BREATHING_STEP_MS_MAX;
    }

    timer_ms = (uint16_t)(state->timer_ms + elapsed_ms);
    state->timer_ms = (uint16_t)timer_ms;
    level = state->level;

    if (timer_ms > step_ms) {
        uint32_t falling = state->falling;

        do {
            timer_ms = (uint16_t)(timer_ms - step_ms);
            if (falling == 0) {
                if (level < RGB_BREATHING_LEVEL_MAX) {
                    level = (uint8_t)(level + RGB_BREATHING_LEVEL_STEP);
                    state->level = (uint8_t)level;
                } else {
                    /* Turn-around: the step is consumed without moving. */
                    falling = 1;
                    state->falling = 1;
                }
            } else {
                if (level != 0) {
                    level = (uint8_t)(level - RGB_BREATHING_LEVEL_STEP);
                    state->level = (uint8_t)level;
                } else {
                    falling = 0;
                    state->falling = 0;
                }
            }
        } while (timer_ms > step_ms);

        state->timer_ms = (uint16_t)timer_ms;
    }

    scale = (uint8_t)(((min_scale << 8) + (uint8_t)(255 * level / RGB_BREATHING_LEVEL_MAX) * (255 - min_scale)) >> 8);

    for (uint32_t row = 0; row != RGB_MATRIX_ROWS; ++row) {
        for (uint32_t col = 0; col != RGB_MATRIX_COLS; ++col) {
            rgb_frame_scale_led(frame, g_rgb_key_to_led[row * RGB_MATRIX_COLS + col], scale);
        }
    }
}
