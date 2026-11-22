/*
 * layout_data.c - definitions of the RAM globals of the layout module: physical variant, matrix
 * lookup tables selected at start-up, layer state.
 *
 * Objects in .data carry their initial value; objects in .bss have no initialiser. The lookup
 * tables of the variant start as NULL and are set by layout_select_variant (start-up step
 * STARTUP_LAYOUT_VARIANT); nothing reads them before that step: the scan interrupt, the key
 * task, the host protocol and the LED frame start later.
 */
#include "layout/layout_data.h"

/* ---- .data ------------------------------------------------------------------------------------ */

/* Variant 1 until the device settings give a number the board has. Matches stock firmware
 * 2.14.1. */
uint8_t g_layout_physical_variant = 1u;

/* ---- .bss ------------------------------------------------------------------------------------- */

const uint8_t *g_layout_keyid_to_scanpos;
const uint8_t *g_layout_scanpos_to_keyid;
uint8_t g_layout_layer_state;
const uint8_t *g_layout_spacebar_fill_cols;
uint8_t g_layout_spacebar_fill_col_count;

/* Object sizes. */
_Static_assert(sizeof(g_layout_physical_variant) == 1, "g_layout_physical_variant size");
_Static_assert(sizeof(g_layout_keyid_to_scanpos) == 4, "g_layout_keyid_to_scanpos size");
_Static_assert(sizeof(g_layout_scanpos_to_keyid) == 4, "g_layout_scanpos_to_keyid size");
_Static_assert(sizeof(g_layout_layer_state) == 1, "g_layout_layer_state size");
_Static_assert(sizeof(g_layout_spacebar_fill_cols) == 4, "g_layout_spacebar_fill_cols size");
_Static_assert(sizeof(g_layout_spacebar_fill_col_count) == 1, "g_layout_spacebar_fill_col_count size");
