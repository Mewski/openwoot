/*
 * rgb_data.h - RAM globals of the rgb module (extern declarations).
 *
 * Definitions: rgb_data.c. Every object below is used from the main loop only (no interrupt
 * handler reaches the rgb module). Re-entrancy caveat: an LED driver write runs tud_task
 * while it waits, so USB callbacks can run in the middle of it.
 *
 * Depends on rgb/rgb_types.h and rgb/rgb_overlay.h for the object types.
 */
#ifndef RGB_DATA_H
#define RGB_DATA_H

#include "rgb/rgb_overlay.h"
#include "rgb/rgb_types.h"

/* ---- .data ------------------------------------------------------------------------------ */

/* Indicator key positions (RGB_KEY_POS, 0xFF = key not on keymap layer 0); written by
 * layout_record_indicator_key, read by rgb_set_lock_indicator. */
extern uint8_t g_rgb_layer_lock_key_pos;  /* first key with code 231 (Fn / layer key) */
extern uint8_t g_rgb_caps_lock_key_pos;   /* first key with code 54 (Caps Lock) */
extern uint8_t g_rgb_scroll_lock_key_pos; /* first key with code 68 (Scroll Lock) */
extern uint8_t g_rgb_num_lock_key_pos;    /* first key with code 80 (Num Lock) */

extern uint8_t g_rgb_fade_level; /* global brightness factor 0..255 driven by fade overlays (type 2) */

extern int8_t g_rgb_dirty_pages[RGB_DIRTY_PAGE_QUEUE_LEN]; /* queue of frame pages to re-send, free entries negative */

/* ---- .bss ------------------------------------------------------------------------------- */

extern uint8_t g_rgb_driver_enabled;       /* 1 while the LED driver should run (USB configured and not suspended) */
extern uint8_t g_rgb_driver_state_changed; /* set by rgb_update_output_enable, consumed by the main-loop tasks */

extern const uint8_t *g_rgb_key_to_led; /* key -> LED table of the layout variant, [row * RGB_MATRIX_COLS + col] = LED number or RGB_LED_NONE; set by layout_select_variant */

extern RgbSettings g_rgb_settings; /* file `rgb` of the active profile */

extern uint16_t g_rgb_base_colors[RGB_MATRIX_ROWS][RGB_BASE_COLOR_COLS]; /* RGB565 base colour per key, file `clr`, columns 0..RGB_MATRIX_COLS - 1 used */

extern uint8_t g_rgb_sdk_mode_active; /* 1 while the host drives the LEDs directly (Wooting RGB SDK commands) */

extern uint32_t g_rgb_frame_stamp_ms; /* uptime of the last rgb_update_frame call (time base of the idle timer) */

extern rgb_idle_timer_t g_rgb_idle_timer; /* time since the last key activity */
extern uint8_t g_rgb_idle_flags;          /* RGB_IDLE_FLAG_* */

extern uint8_t g_rgb_effect_layer_count; /* used entries of g_rgb_effect_layers, 0..6 */
extern uint8_t g_rgb_layer_color_count;  /* number of consecutive layer colour maps loaded (files r1..r3) */

extern uint16_t g_rgb_layer_colors[RGB_COLOR_LAYER_COUNT][RGB_MATRIX_ROWS][RGB_MATRIX_COLS]; /* RGB565 colour maps of layers 1..3, files r1..r3 */

extern uint8_t g_rgb_layer_switch_pending; /* 1 = rgb_update_frame must switch the key colours to g_rgb_requested_layer */
extern uint8_t g_rgb_shown_color_layer;    /* colour map painted into g_rgb_frame_main: 0 = base, 1..3 = layer map */
extern uint8_t g_rgb_force_render;         /* 1 = render on the next call regardless of the 16 ms period */

extern rgb_frame_t g_rgb_frame_main; /* main LED frame; writes through rgb_frame_set_led queue dirty pages */

extern uint8_t g_rgb_overlay_count; /* used entries of g_rgb_overlay_slots, 0..5 */

extern uint32_t g_rgb_effect_stamp_ms; /* uptime of the last render pass (effects, overlays, optional features) */

extern rgb_overlay_slot_t g_rgb_overlay_slots[RGB_OVERLAY_SLOT_COUNT]; /* timed indicators drawn on top of the frame */

extern rgb_effect_layer_t g_rgb_effect_layers[RGB_EFFECT_LAYER_MAX]; /* effect layers: settings copy and animation state */

extern uint8_t g_rgb_applied_brightness; /* copy of the settings brightness used by the flush step */

extern const uint16_t *g_rgb_shown_layer_colors; /* RGB_MATRIX_ROWS x RGB_MATRIX_COLS map of the shown layer inside g_rgb_layer_colors, or NULL */

extern uint8_t g_rgb_effect_redraw_request; /* set by rgb_paint_base_colors, cleared by rgb_render_effect_layers */
extern uint8_t g_rgb_requested_layer;       /* colour layer wanted by the layer key handlers, 0 = base */
extern uint8_t g_rgb_effect_reset_request;  /* non-zero = re-initialise all layer states; nothing in this firmware sets it */

extern uint8_t g_rgb_overlay_dirty_mask; /* RGB_OVERLAY_CLASS_* bits requested by rgb_overlay_start */
extern uint8_t g_rgb_polar_tables_ready; /* 1 once the two tables below are computed */

extern uint8_t g_rgb_key_distance_table[RGB_MATRIX_ROWS][RGB_MATRIX_COLS]; /* 16 * distance from (row 3.0, col 8.5) */
extern uint8_t g_rgb_key_angle_table[RGB_MATRIX_ROWS][RGB_MATRIX_COLS];    /* (atan2f(3.0 - row, 8.5 - col) / 2 pi + 0.5) * 255 */

#endif /* RGB_DATA_H */
