/*
 * layout_keycode_tables.h - constant tables of module `layout` that are the same on every board:
 * the action word of every key code and the "no key" position value. Each table is described at
 * its definition in layout_keycode_tables.c.
 *
 * Depends on nothing; the per-board layout data is in the board's tables/layout_tables.h.
 */
#ifndef TABLES_LAYOUT_KEYCODE_TABLES_H
#define TABLES_LAYOUT_KEYCODE_TABLES_H

#include <stdint.h>

extern const uint16_t k_layout_keycode_actions[256];
extern const uint8_t k_layout_no_key_pos;

#endif /* TABLES_LAYOUT_KEYCODE_TABLES_H */
