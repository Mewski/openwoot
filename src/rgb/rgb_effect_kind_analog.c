/*
 * rgb_effect_kind_analog.c - effect kinds 8 (analog spotlight) and 9 (analog glow). Both read
 * the per-key analog levels 0..255 that rgb_effects_feed_key_travel keeps in the
 * layer state, build a key matrix mask from them and dim the LEDs already in the frame.
 */
#include <string.h>

#include "rgb/rgb_effect_layers_internal.h"
#include "rgb/rgb_frame.h"

#define SPOTLIGHT_LEVELS_PER_KEY 30 /* the lit square grows by one key per 30 levels (0..8) */

/* Stores level at (row, col) of the mask if the position is inside the matrix. */
static void spotlight_mark(uint8_t mask[RGB_MATRIX_ROWS][RGB_MATRIX_COLS], int32_t row, int32_t col, uint8_t level)
{
    if (row >= 0 && row < RGB_MATRIX_ROWS && col >= 0 && col < RGB_MATRIX_COLS) {
        mask[row][col] = level;
    }
}

/*
 * Kind 8: a square around every pressed key keeps its colour, everything else is dimmed to
 * the base level. The square has the half-size level / 30 - 1 keys, so a key lights itself
 * from level 30 on. Positions outside the matrix are clipped. Later keys overwrite earlier
 * ones, which only matters for "mask != 0".
 */
void rgb_effect_kind_analog_spotlight(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    uint8_t                     base_level = layer->cfg.effect.analog_spotlight.base_level;
    rgb_effect_key_map_state_t *state      = &layer->state.key_map;
    uint8_t                     mask[RGB_MATRIX_ROWS][RGB_MATRIX_COLS];

    memset(mask, 0, sizeof(mask));
    for (int32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (int32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            uint8_t level = state->cells[row][col];
            int32_t reach = level / SPOTLIGHT_LEVELS_PER_KEY;

            for (int32_t row_offset = 0; row_offset < reach; row_offset++) {
                for (int32_t col_offset = 0; col_offset < reach; col_offset++) {
                    spotlight_mark(mask, row + row_offset, col + col_offset, level);
                    spotlight_mark(mask, row - row_offset, col - col_offset, level);
                    spotlight_mark(mask, row + row_offset, col - col_offset, level);
                    spotlight_mark(mask, row - row_offset, col + col_offset, level);
                }
            }
        }
    }

    /* Keys inside a square keep their colour, all others are dimmed. */
    for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            rgb_frame_scale_led(frame, rgb_effect_key_led(row, col), mask[row][col] != 0 ? 255u : base_level);
        }
    }
    state->dirty = 0;
}

/*
 * Kind 9: every pressed key and its eight neighbours are lit in proportion to the key depth
 * (3 x 3 maximum of the levels), unlit keys are dimmed to the base level.
 */
void rgb_effect_kind_analog_glow(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    uint8_t                     base_level = layer->cfg.effect.analog_glow.base_level;
    rgb_effect_key_map_state_t *state      = &layer->state.key_map;
    uint8_t                     mask[RGB_MATRIX_ROWS][RGB_MATRIX_COLS];

    memset(mask, 0, sizeof(mask));
    for (int32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (int32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            uint8_t level = state->cells[row][col];

            if (level == 0) {
                continue;
            }
            for (int32_t near_row = row - 1; near_row <= row + 1; near_row++) {
                if (near_row < 0 || near_row >= RGB_MATRIX_ROWS) {
                    continue;
                }
                for (int32_t near_col = col - 1; near_col <= col + 1; near_col++) {
                    if (near_col < 0 || near_col >= RGB_MATRIX_COLS) {
                        continue;
                    }
                    if (mask[near_row][near_col] < level) {
                        mask[near_row][near_col] = level;
                    }
                }
            }
        }
    }

    /* Scale between the base level (mask 0) and full brightness (mask 255). */
    for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            uint8_t level = mask[row][col];
            uint8_t scale = base_level;

            if (level != 0) {
                scale = (uint8_t)(((base_level << 8) + (255 - base_level) * level) >> 8);
            }
            rgb_frame_scale_led(frame, rgb_effect_key_led(row, col), scale);
        }
    }
    state->dirty = 0;
}
