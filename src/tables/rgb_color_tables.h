/*
 * rgb_color_tables.h - constant tables of module `rgb` that are the same on every board: the
 * gamma curve of the output stage, the seven-colour effect palette, the permutation of the
 * simplex noise effect and the overlay id of each lock indicator. Each table is described at
 * its definition in rgb_color_tables.c.
 *
 * Depends on nothing; the per-board lighting data is in the board's tables/rgb_tables.h.
 */
#ifndef TABLES_RGB_COLOR_TABLES_H
#define TABLES_RGB_COLOR_TABLES_H

#include <stdint.h>

extern const uint8_t k_rgb_gamma_table[256];
extern const uint8_t k_rgb_palette_red[7];
extern const uint8_t k_rgb_palette_green[7];
extern const uint8_t k_rgb_palette_blue[7];
extern const uint8_t k_rgb_simplex_perm[256];
extern const uint8_t k_rgb_lock_indicator_overlay_ids[4];

#endif /* TABLES_RGB_COLOR_TABLES_H */
