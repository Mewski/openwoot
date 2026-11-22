/*
 * layout_data.h - RAM globals of the layout module: physical variant, matrix lookup tables
 * selected at start-up, layer state.
 *
 * One extern per global. Definitions and .data initial values: layout_data.c.
 */
#ifndef LAYOUT_DATA_H
#define LAYOUT_DATA_H

#include "layout/layout_types.h"

/* Physical layout variant number, below BOARD_LAYOUT_VARIANT_COUNT. Loaded from the device settings
 * (settings_load_and_apply); layout_select_variant selects the tables of k_layout_variants with it. */
extern uint8_t g_layout_physical_variant;

/* -> LAYOUT_ROWS x LAYOUT_COLS table [row * LAYOUT_COLS + col] = scan position (index of
 * g_analog_key_states) or LAYOUT_NO_KEY; table of the variant (layout_select_variant). */
extern const uint8_t *g_layout_keyid_to_scanpos;

/* -> LAYOUT_SCAN_POSITIONS table [mux address + BOARD_MUX_POSITIONS * mux index] = key id
 * (row << 5 | col) or LAYOUT_NO_KEY; table of the variant (layout_select_variant). */
extern const uint8_t *g_layout_scanpos_to_keyid;

/* Layer state, LAYOUT_STATE_*: bits 1:0 highest keymap layer present, bits 3:2 active layer, bits
 * 5:4 locked layer, bit 6 layer 0 loaded from the profile itself. */
extern uint8_t g_layout_layer_state;

/* -> list of row-5 columns that mirror the space bar value (rgb_effect_copy_spacebar_cell); NULL
 * for a variant number the board does not have. Set by layout_select_variant. */
extern const uint8_t *g_layout_spacebar_fill_cols;

/* Entries in g_layout_spacebar_fill_cols. */
extern uint8_t g_layout_spacebar_fill_col_count;

#endif /* LAYOUT_DATA_H */
