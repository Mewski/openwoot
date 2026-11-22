/*
 * rgb_effect_kind_key_test.c - effect kind 11: keys marked red or green by
 * rgb_effects_handle_key_event, every other key dimmed to the base level.
 */
#include "rgb/rgb_effect_layers_internal.h"
#include "rgb/rgb_frame.h"

#define KEY_TEST_MARK_RED   1
#define KEY_TEST_MARK_GREEN 2

/* Kind 11 renderer: draws the marks of the key map (1 red, 2 green) and clears the dirty flag. */
void rgb_effect_kind_key_test(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    uint8_t                     base_level = layer->cfg.effect.key_test.base_level;
    rgb_effect_key_map_state_t *state      = &layer->state.key_map;

    for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            uint8_t mark = state->cells[row][col];

            if (mark > KEY_TEST_MARK_GREEN) {
                /* Unknown mark: reset the cell and draw it as idle. */
                state->cells[row][col] = 0;
                rgb_frame_scale_led(frame, rgb_effect_key_led(row, col), base_level);
            } else if (mark == KEY_TEST_MARK_RED) {
                rgb_frame_set_led(frame, rgb_effect_key_led(row, col), 255, 0, 0);
            } else if (mark == KEY_TEST_MARK_GREEN) {
                rgb_frame_set_led(frame, rgb_effect_key_led(row, col), 0, 255, 0);
            } else {
                rgb_frame_scale_led(frame, rgb_effect_key_led(row, col), base_level);
            }
        }
    }
    state->dirty = 0;
}
