/*
 * keys_gamepad_map.c - per-key gamepad bindings of profile file "gm".
 */
#include "keys/keys.h"
#include "analog/analog_data.h"
#include "hid/hid_data.h"
#include "layout/layout.h"

/* Applies profile file "gm": clears the gamepad binding of every key record, then stores the
 * binding code of each mapped key (matrix format: one byte per row / column; list format: items
 * of key id | code << 8) and counts the mapped keys in g_hid_gamepad_mapped_key_count. Also
 * takes over the "gamepad keys exclusive" flag of the profile. */
void keys_apply_gamepad_map(const GamepadMapping *mapping)
{
    const KeyByteMap *bindings = &mapping->bindings;

    for (uint32_t pos = 0; pos != ANALOG_KEY_COUNT; pos++) {
        g_analog_key_states[pos].gamepad_binding = 0;
    }
    g_hid_gamepad_mapped_key_count = 0;
    g_hid_gamepad_keys_exclusive = mapping->gamepad_keys_exclusive;

    if (bindings->which_map == KeyByteMap_matrix_tag) {
        for (uint32_t row = 0; row != LAYOUT_ROWS; row++) {
            for (uint32_t col = 0; col != LAYOUT_KEYMAP_COLS; col++) {
                uint8_t code = bindings->map.matrix.rows[row].values[col];

                if (code == 0) {
                    continue;
                }
                if (col <= LAYOUT_COLS - 1u) {
                    uint8_t pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];

                    if (pos != ANALOG_NO_SCAN_POS) {
                        g_analog_key_states[pos].gamepad_binding = code;
                    }
                }
                g_hid_gamepad_mapped_key_count++;
            }
        }
    } else if (bindings->which_map == KeyByteMap_list_tag) {
        uint32_t item_count = bindings->map.list.items_count;

        /* The list length is taken as the number of mapped keys, also for items without code. */
        g_hid_gamepad_mapped_key_count = (uint8_t)item_count;

        /* The index is 8 bit wide. */
        for (uint8_t i = 0; i < item_count; i++) {
            uint16_t item = bindings->map.list.items[i];
            uint8_t key_id = (uint8_t)item;
            uint8_t code = (uint8_t)(item >> 8);
            uint8_t col = KEYS_KEY_ID_COL(key_id);

            if (code == 0 || key_id > KEYS_KEY_ID_MAX || col > LAYOUT_COLS - 1u) {
                continue;
            }
            uint8_t pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * KEYS_KEY_ID_ROW(key_id) + col];
            if (pos != ANALOG_NO_SCAN_POS) {
                g_analog_key_states[pos].gamepad_binding = code;
            }
        }
    }
}
