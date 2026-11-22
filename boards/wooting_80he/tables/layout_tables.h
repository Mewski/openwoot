/*
 * layout_tables.h - layout data of the board: the names of its physical layout variants, the
 * factory keymap layers and, per variant, the scan position <-> key maps, the space-bar fill
 * columns and the key codes the variant writes into the factory keymap. Each table is described
 * at its definition in layout_tables.c.
 *
 * Depends on board.h for the dimensions and on layout/layout_types.h for the element types.
 */
#ifndef TABLES_LAYOUT_TABLES_H
#define TABLES_LAYOUT_TABLES_H

#include <stdint.h>

#include "board.h"
#include "layout/layout_types.h"

/* Physical layout variants of this board: the variant number stored in the device settings and
 * the index of k_layout_variants. */
enum {
    LAYOUT_VARIANT_ANSI = 0,
    LAYOUT_VARIANT_ISO  = 1,
    LAYOUT_VARIANT_JIS  = 2,
};

extern const uint8_t k_layout_default_keymap_layer0[BOARD_KEY_ROWS][BOARD_KEYMAP_COLS];
extern const uint8_t k_layout_default_keymap_layer1[BOARD_KEY_ROWS][BOARD_KEYMAP_COLS];
extern const uint8_t k_layout_spacebar_fill_cols_iso[4];
extern const uint8_t k_layout_scanpos_to_keyid_iso[BOARD_MUX_COUNT][BOARD_MUX_POSITIONS];
extern const uint8_t k_layout_keyid_to_scanpos_iso[BOARD_KEY_ROWS][BOARD_KEY_COLS];
extern const uint8_t k_layout_spacebar_fill_cols_ansi[4];
extern const uint8_t k_layout_scanpos_to_keyid_ansi[BOARD_MUX_COUNT][BOARD_MUX_POSITIONS];
extern const uint8_t k_layout_keyid_to_scanpos_ansi[BOARD_KEY_ROWS][BOARD_KEY_COLS];
extern const uint8_t k_layout_spacebar_fill_cols_jis[2];
extern const uint8_t k_layout_scanpos_to_keyid_jis[BOARD_MUX_COUNT][BOARD_MUX_POSITIONS];
extern const uint8_t k_layout_keyid_to_scanpos_jis[BOARD_KEY_ROWS][BOARD_KEY_COLS];
extern const layout_keymap_patch_t k_layout_layer0_patches_jis[2];

extern const uint8_t *const k_layout_default_keymap_layers[BOARD_DEFAULT_KEYMAP_LAYERS];
extern const layout_variant_t k_layout_variants[BOARD_LAYOUT_VARIANT_COUNT];

#endif /* TABLES_LAYOUT_TABLES_H */
