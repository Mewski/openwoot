/* Constant tables of module `profile` (declarations); each table is described at its definition in
 * profile_tables.c. */
#ifndef TABLES_PROFILE_TABLES_H
#define TABLES_PROFILE_TABLES_H

#include <stdint.h>

#include "board.h"

extern const uint16_t k_profile_default_colors_a[BOARD_KEY_ROWS][BOARD_KEYMAP_COLS];
extern const uint16_t k_profile_default_colors_b[BOARD_KEY_ROWS][BOARD_KEYMAP_COLS];
extern const uint16_t k_profile_default_layer1_colors[BOARD_KEY_ROWS][BOARD_KEY_COLS];

#endif /* TABLES_PROFILE_TABLES_H */
