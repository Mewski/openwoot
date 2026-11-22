/*
 * layout.h - public functions of the layout module: physical layout variant, keymap layers and
 * special-key (Fn) action handlers.
 *
 * Depends on board.h (through layout_types.h); the data comes from the board's layout tables.
 */
#ifndef LAYOUT_LAYOUT_H_
#define LAYOUT_LAYOUT_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "layout/layout_types.h"
#include "layout/layout_data.h"

/* Special-key action handlers, index = enum layout_key_action (entry 0 is NULL). */
extern const layout_key_action_handler_t k_layout_key_action_handlers[LAYOUT_KEY_ACTION_COUNT];

int layout_key_action_brightness_up(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);
int layout_key_action_brightness_down(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);
int layout_key_action_toggle_mode_profile(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);
int layout_key_action_none(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);
int layout_key_action_next_profile(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);
void *layout_get_default_keymap(uint32_t layer, uint8_t *keymap);
void layout_select_variant(void);
void layout_record_indicator_key(uint32_t key_code, uint32_t row, uint32_t col);
void layout_request_profile_switch(uint32_t row, uint32_t col, uint32_t profile_index);
int layout_key_action_select_profile_1(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);
int layout_key_action_select_profile_1_pressed(uint32_t row, uint32_t col);
int layout_key_action_select_profile_2(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);
int layout_key_action_select_profile_2_pressed(uint32_t row, uint32_t col);
int layout_key_action_select_profile_3(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);
int layout_key_action_select_profile_3_pressed(uint32_t row, uint32_t col);
int layout_key_action_select_profile_0(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);
int layout_key_action_select_profile_0_pressed(uint32_t row, uint32_t col);
int layout_key_action_toggle_key_disable(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);
void layout_apply_keymap_layer(const uint8_t *keymap, uint32_t layer);
void layout_load_keymap_layers(uint16_t profile_id);
int layout_key_action_toggle_layer_lock(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed);
int layout_key_action_momentary_layer1(uint32_t row, uint32_t col, uint32_t arg, uint32_t pressed);
int layout_key_action_momentary_layer2(uint32_t row, uint32_t col, uint32_t arg, uint32_t pressed);
int layout_key_action_momentary_layer3(uint32_t row, uint32_t col, uint32_t arg, uint32_t pressed);

#endif /* LAYOUT_LAYOUT_H_ */
