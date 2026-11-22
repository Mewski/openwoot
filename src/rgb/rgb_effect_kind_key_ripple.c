/*
 * rgb_effect_kind_key_ripple.c - effect kind 5: a diamond-shaped wave front spreads from every
 * pressed key and leaves a fading tail. Cellular automaton on the key matrix; key presses
 * enter through rgb_effects_handle_key_event (cell = colour, direction "new").
 *
 * Cell byte (rgb_effect_ripple_state_t.cells): bits 2:0 colour (0 = empty, 1..7 = palette
 * index + 1), bit 3 tail, bits 7:4 direction code of a wave front ("head") or fade level
 * 15..0 of a tail.
 */
#include <stdbool.h>
#include <string.h>

#include "rgb/rgb_effect_layers_internal.h"
#include "rgb/rgb_frame.h"
#include "tables/rgb_color_tables.h"

/* RgbEffectKeyRipple.flags */
#define RIPPLE_FLAG_PALETTE 0x01 /* draw cells in their palette colour instead of dimming the frame */
#define RIPPLE_FLAG_INVERT  0x04 /* idle keys untouched, wave front at the base level */

/* Direction code of a head (cell bits 7:4). */
enum ripple_direction {
    RIPPLE_DIR_NEW        = 0, /* just pressed: waits for the last move step of the frame */
    RIPPLE_DIR_BURST      = 1, /* spawns the four fronts */
    RIPPLE_DIR_UP_LEFT    = 2,
    RIPPLE_DIR_UP_RIGHT   = 3,
    RIPPLE_DIR_DOWN_LEFT  = 4,
    RIPPLE_DIR_DOWN_RIGHT = 5
};

#define RIPPLE_STEPS_PER_SECOND_MAX 50
#define RIPPLE_LEVEL_MASK           0xF0
#define RIPPLE_LEVEL_MAX            15
#define RIPPLE_LEVEL_STEP           (1 << RGB_RIPPLE_CELL_LEVEL_SHIFT)
#define RIPPLE_TAIL_FULL            0xF8 /* tail flag with level 15 */

typedef uint8_t ripple_grid_t[RGB_MATRIX_ROWS][RGB_MATRIX_COLS];

/* A head may move into a cell that is empty or holds a tail. */
static bool ripple_cell_is_free(uint8_t cell)
{
    return (cell & RGB_RIPPLE_CELL_TAIL) != 0 || (cell & RGB_RIPPLE_CELL_COLOR_MASK) == 0;
}

/* Returns the cell as a head with the given direction code (colour and tail bit kept). */
static uint8_t ripple_head(uint8_t cell, enum ripple_direction direction)
{
    return (uint8_t)((cell & 0x0F) | (direction << RGB_RIPPLE_CELL_LEVEL_SHIFT));
}

/* Adds elapsed_ms to a timer and returns how many whole periods it now holds (8-bit count). */
static uint8_t ripple_take_steps(uint16_t *timer_ms, uint32_t period_ms, uint32_t elapsed_ms)
{
    uint16_t timer = (uint16_t)(*timer_ms + elapsed_ms);
    uint8_t  steps = 0;

    while (timer > period_ms) {
        timer = (uint16_t)(timer - period_ms);
        steps++;
    }
    *timer_ms = timer;
    return steps;
}

/* Copies a head into next at (row, col) if that position is free (empty or a tail). */
static void ripple_enter_cell(ripple_grid_t next, uint32_t row, uint32_t col, uint8_t head)
{
    if (ripple_cell_is_free(next[row][col])) {
        next[row][col] = head;
    }
}

/*
 * Moves a diagonal front: first into its vertical neighbour (above if up, else below), then
 * into its horizontal neighbour (left if left, else right). The copies keep the direction.
 */
static void ripple_advance_front(ripple_grid_t next, uint32_t row, uint32_t col, uint8_t head, bool up, bool left)
{
    if (up) {
        if (row > 0) {
            ripple_enter_cell(next, row - 1, col, head);
        }
    } else if (row < RGB_MATRIX_ROWS - 1) {
        ripple_enter_cell(next, row + 1, col, head);
    }

    if (left) {
        if (col > 0) {
            ripple_enter_cell(next, row, col - 1, head);
        }
    } else if (col < RGB_MATRIX_COLS - 1) {
        ripple_enter_cell(next, row, col + 1, head);
    }
}

/*
 * One move step: every head of cells advances into next (burst: four unconditional copies,
 * fronts: copies of themselves into the free neighbours of their two directions) and turns
 * into a tail of level 15; tails are kept while their level is above 0.
 */
static void ripple_move(ripple_grid_t cells, ripple_grid_t next, bool last_step)
{
    for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            uint8_t cell      = cells[row][col];
            bool    has_up    = row > 0;
            bool    has_down  = row < RGB_MATRIX_ROWS - 1;
            bool    has_left  = col > 0;
            bool    has_right = col < RGB_MATRIX_COLS - 1;

            if ((cell & RGB_RIPPLE_CELL_COLOR_MASK) == 0) {
                continue;
            }
            if (cell & RGB_RIPPLE_CELL_TAIL) {
                if (next[row][col] == 0 && (cell & RIPPLE_LEVEL_MASK) != 0) {
                    next[row][col] = cell;
                }
                continue;
            }

            switch (cell >> RGB_RIPPLE_CELL_LEVEL_SHIFT) {
            case RIPPLE_DIR_NEW:
                next[row][col] = cell;
                if (last_step) {
                    next[row][col] = ripple_head(next[row][col], RIPPLE_DIR_BURST);
                }
                continue;

            case RIPPLE_DIR_BURST:
                if (has_up) {
                    next[row - 1][col] = ripple_head(cell, RIPPLE_DIR_UP_LEFT);
                }
                if (has_left) {
                    next[row][col - 1] = ripple_head(cell, RIPPLE_DIR_DOWN_LEFT);
                }
                if (has_down) {
                    next[row + 1][col] = ripple_head(cell, RIPPLE_DIR_DOWN_RIGHT);
                }
                if (has_right) {
                    next[row][col + 1] = ripple_head(cell, RIPPLE_DIR_UP_RIGHT);
                }
                break;

            case RIPPLE_DIR_UP_LEFT:
                ripple_advance_front(next, row, col, cell, true, true);
                break;
            case RIPPLE_DIR_UP_RIGHT:
                ripple_advance_front(next, row, col, cell, true, false);
                break;
            case RIPPLE_DIR_DOWN_LEFT:
                ripple_advance_front(next, row, col, cell, false, true);
                break;
            case RIPPLE_DIR_DOWN_RIGHT:
                ripple_advance_front(next, row, col, cell, false, false);
                break;

            default: /* direction codes 6..15 are never written; such a head just turns into a tail */
                break;
            }
            next[row][col] = cell | RIPPLE_TAIL_FULL;
        }
    }
}

/* One decay step: every tail with a level above 0 loses one level. */
static void ripple_decay(ripple_grid_t cells)
{
    for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            uint8_t cell = cells[row][col];

            if ((cell & RGB_RIPPLE_CELL_COLOR_MASK) != 0 && (cell & RGB_RIPPLE_CELL_TAIL) != 0 &&
                (cell & RIPPLE_LEVEL_MASK) != 0) {
                cells[row][col] = (uint8_t)(cell - RIPPLE_LEVEL_STEP);
            }
        }
    }
}

/*
 * Draws the grid and returns 1 if any cell is non-empty. Normal mode: idle keys dimmed to
 * base_level, front at full level, tail fading from 255 to base_level. Inverted: idle keys
 * untouched, front at base_level, tail rising back to 255.
 */
static uint8_t ripple_draw(rgb_frame_t *frame, ripple_grid_t cells, uint8_t flags, uint8_t base_level)
{
    bool    palette = (flags & RIPPLE_FLAG_PALETTE) != 0;
    bool    invert  = (flags & RIPPLE_FLAG_INVERT) != 0;
    uint8_t active  = 0;

    for (uint32_t row = 0; row < RGB_MATRIX_ROWS; row++) {
        for (uint32_t col = 0; col < RGB_MATRIX_COLS; col++) {
            uint8_t cell = cells[row][col];
            int32_t fade = 255 * (cell >> RGB_RIPPLE_CELL_LEVEL_SHIFT) / RIPPLE_LEVEL_MAX; /* tail level as 0..255 */
            uint8_t level;

            if ((cell & RGB_RIPPLE_CELL_COLOR_MASK) == 0) {
                if (!invert) {
                    rgb_frame_scale_led(frame, rgb_effect_key_led(row, col), base_level);
                }
                continue;
            }

            if (invert) {
                if (cell & RGB_RIPPLE_CELL_TAIL) {
                    level = (uint8_t)((uint16_t)(fade * (base_level - 255) - 256) >> 8);
                } else {
                    level = base_level;
                }
            } else {
                if (cell & RGB_RIPPLE_CELL_TAIL) {
                    level = (uint8_t)((uint16_t)((base_level << 8) + fade * (255 - base_level)) >> 8);
                } else {
                    level = 255;
                }
            }

            if (palette) {
                uint8_t color = (uint8_t)((cell & RGB_RIPPLE_CELL_COLOR_MASK) - 1);
                uint8_t r     = 0;
                uint8_t g     = 0;
                uint8_t b     = 0;

                if (color < RGB_PALETTE_COLOR_COUNT) {
                    r = k_rgb_palette_red[color];
                    g = k_rgb_palette_green[color];
                    b = k_rgb_palette_blue[color];
                }
                if (level != 255) {
                    r = (uint8_t)((level + r * level) >> 8);
                    g = (uint8_t)((level + g * level) >> 8);
                    b = (uint8_t)((level + b * level) >> 8);
                }
                rgb_frame_set_led(frame, rgb_effect_key_led(row, col), r, g, b);
            } else {
                rgb_frame_scale_led(frame, rgb_effect_key_led(row, col), level);
            }
            active = 1;
        }
    }
    return active;
}

/*
 * Kind 5 renderer: runs the move steps (steps_per_second, 1..50) and decay steps
 * (decay_step_ms) that fit into elapsed_ms, then draws the grid and updates state->active.
 */
void rgb_effect_kind_key_ripple(rgb_frame_t *frame, rgb_effect_layer_t *layer, uint32_t elapsed_ms)
{
    const RgbEffectKeyRipple  *params = &layer->cfg.effect.key_ripple;
    rgb_effect_ripple_state_t *state  = &layer->state.ripple;
    ripple_grid_t              next;

    uint32_t steps_per_second = params->steps_per_second;
    uint32_t decay_period_ms  = params->decay_step_ms;

    if (steps_per_second == 0) {
        steps_per_second = 1;
    }
    if (steps_per_second >= RIPPLE_STEPS_PER_SECOND_MAX) {
        steps_per_second = RIPPLE_STEPS_PER_SECOND_MAX;
    }
    if (decay_period_ms == 0) {
        decay_period_ms = 1;
    }

    uint8_t moves  = ripple_take_steps(&state->move_timer_ms, 1000 / steps_per_second, elapsed_ms);
    uint8_t decays = ripple_take_steps(&state->decay_timer_ms, decay_period_ms, elapsed_ms);

    /*
     * KNOWN ISSUE: the scratch grid is cleared once per frame, not once per move step. From the
     * second move step of a frame on it still holds the previous step's result, so expired
     * tails survive until the next frame and heads are tested against stale cells. Needs more
     * than one step per 16 ms frame, which the limit of 50 steps per second prevents unless
     * frames are late. Matches stock firmware 2.14.1.
     */
    memset(next, 0, sizeof(next));
    for (uint8_t step = 0; step < moves; step++) {
        ripple_move(state->cells, next, step == moves - 1);
        memcpy(state->cells, next, sizeof(next));
    }
    for (uint8_t step = 0; step < decays; step++) {
        ripple_decay(state->cells);
    }

    state->active = ripple_draw(frame, state->cells, params->flags, params->base_level);
}
