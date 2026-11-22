/*
 * rgb_data.c - definitions of the RAM globals of the rgb module (declarations and
 * descriptions: rgb_data.h). Objects with an initializer are placed in .data, the others
 * in .bss.
 */
#include "rgb/rgb_data.h"

/* ---- .data -------------------------------------------------------------------------------- */

uint8_t g_rgb_layer_lock_key_pos = RGB_KEY_POS_NONE;
uint8_t g_rgb_caps_lock_key_pos = RGB_KEY_POS_NONE;
uint8_t g_rgb_scroll_lock_key_pos = RGB_KEY_POS_NONE;
uint8_t g_rgb_num_lock_key_pos = RGB_KEY_POS_NONE;

uint8_t g_rgb_fade_level = RGB_FADE_LEVEL_FULL;

int8_t g_rgb_dirty_pages[RGB_DIRTY_PAGE_QUEUE_LEN] = {
    [0 ... RGB_DIRTY_PAGE_QUEUE_LEN - 1] = RGB_DIRTY_PAGE_FREE,
};

/* ---- .bss --------------------------------------------------------------------------------- */

uint8_t g_rgb_driver_enabled;
uint8_t g_rgb_driver_state_changed;

const uint8_t *g_rgb_key_to_led;

RgbSettings g_rgb_settings;

uint16_t g_rgb_base_colors[RGB_MATRIX_ROWS][RGB_BASE_COLOR_COLS];

uint8_t g_rgb_sdk_mode_active;

uint32_t g_rgb_frame_stamp_ms;

rgb_idle_timer_t g_rgb_idle_timer;
uint8_t g_rgb_idle_flags;

uint8_t g_rgb_effect_layer_count;
uint8_t g_rgb_layer_color_count;

uint16_t g_rgb_layer_colors[RGB_COLOR_LAYER_COUNT][RGB_MATRIX_ROWS][RGB_MATRIX_COLS];

uint8_t g_rgb_layer_switch_pending;
uint8_t g_rgb_shown_color_layer;
uint8_t g_rgb_force_render;

rgb_frame_t g_rgb_frame_main;

uint8_t g_rgb_overlay_count;

uint32_t g_rgb_effect_stamp_ms;

rgb_overlay_slot_t g_rgb_overlay_slots[RGB_OVERLAY_SLOT_COUNT];

rgb_effect_layer_t g_rgb_effect_layers[RGB_EFFECT_LAYER_MAX];

uint8_t g_rgb_applied_brightness;

const uint16_t *g_rgb_shown_layer_colors;

uint8_t g_rgb_effect_redraw_request;
uint8_t g_rgb_requested_layer;
uint8_t g_rgb_effect_reset_request;

uint8_t g_rgb_overlay_dirty_mask;
uint8_t g_rgb_polar_tables_ready;

uint8_t g_rgb_key_distance_table[RGB_MATRIX_ROWS][RGB_MATRIX_COLS];
uint8_t g_rgb_key_angle_table[RGB_MATRIX_ROWS][RGB_MATRIX_COLS];

/* Object sizes, as in stock firmware 2.14.1 of each board. g_rgb_settings does not depend on the
 * board; the others follow from the key matrix and the frame layout of board.h and from the
 * light bar of feature `lightbar` (overlay slots). */
_Static_assert(sizeof(g_rgb_settings) == 132, "g_rgb_settings");
_Static_assert(sizeof(g_rgb_dirty_pages) == RGB_FRAME_PAGE_COUNT, "g_rgb_dirty_pages");
_Static_assert(sizeof(g_rgb_base_colors) == RGB_MATRIX_ROWS * RGB_BASE_COLOR_COLS * sizeof(uint16_t),
               "g_rgb_base_colors");
_Static_assert(sizeof(g_rgb_layer_colors) == RGB_COLOR_LAYER_COUNT * RGB_KEY_COUNT * sizeof(uint16_t),
               "g_rgb_layer_colors");
_Static_assert(sizeof(g_rgb_frame_main) == RGB_FRAME_PAGE_COUNT * RGB_FRAME_PAGE_SIZE, "g_rgb_frame_main");
_Static_assert(sizeof(g_rgb_overlay_slots) == RGB_OVERLAY_SLOT_COUNT * RGB_OVERLAY_SLOT_SIZE, "g_rgb_overlay_slots");
_Static_assert(sizeof(g_rgb_effect_layers) == RGB_EFFECT_LAYER_MAX * (0x10 + RGB_EFFECT_STATE_SIZE),
               "g_rgb_effect_layers");
_Static_assert(sizeof(g_rgb_key_distance_table) == RGB_KEY_COUNT, "g_rgb_key_distance_table");
_Static_assert(sizeof(g_rgb_key_angle_table) == RGB_KEY_COUNT, "g_rgb_key_angle_table");
