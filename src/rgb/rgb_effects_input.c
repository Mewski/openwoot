/*
 * rgb_effects_input.c - key input of the reactive effect layers: key events (press, release,
 * actuation) for the ripple, reactive fade and key test effects, and analog key travel for
 * the level meter, analog spotlight / glow and analog RGB fill effects.
 *
 * Callers: keys_process_scan_pass and keys_update_press_state (never
 * while g_rgb_sdk_mode_active).
 */
#include "rgb/rgb_effects.h"

#include "core/clock.h"
#include "hid/hid.h"
#include "layout/layout_data.h"
#include "usb/usb_data.h"

#define RGB_SPACEBAR_ROW 5
#define RGB_SPACEBAR_COL 6

/* Analog RGB fill (kind 10): the last three keys of the bottom row drive the three colour channels. */
#define RGB_FILL_ROW       (RGB_MATRIX_ROWS - 1)
#define RGB_FILL_RED_COL   (RGB_MATRIX_COLS - 3)
#define RGB_FILL_GREEN_COL (RGB_MATRIX_COLS - 2)
#define RGB_FILL_BLUE_COL  (RGB_MATRIX_COLS - 1)

#define RGB_TRAVEL_FULL            4095 /* key travel range */
#define RGB_LEVEL_DEADZONE         5    /* 8-bit levels up to this value count as released */
#define RGB_LEVEL_METER_DEADZONE   80   /* raw travel up to this value counts as released (kind 7) */

/* Cell of the reactive fade effect in palette mode: level 31 in bits 7:3, colour in bits 2:0. */
#define RGB_REACTIVE_CELL_PALETTE_PRESSED 0xF8

/* Marks of the key test effect (kind 11). */
#define RGB_KEY_TEST_IDLE     0
#define RGB_KEY_TEST_ACTUATED 1 /* drawn red */
#define RGB_KEY_TEST_PRESSED  2 /* drawn green */

/*
 * Copies the cell of the spacebar key (row 5, column 6) of a key matrix map to the other
 * columns under the spacebar (g_layout_spacebar_fill_cols), so the whole bar shows the effect.
 * Does nothing for any other key.
 */
void rgb_effect_copy_spacebar_cell(uint8_t *key_map, uint32_t row, uint32_t col)
{
    if (row == RGB_SPACEBAR_ROW && col == RGB_SPACEBAR_COL && g_layout_spacebar_fill_cols != NULL) {
        uint32_t count = g_layout_spacebar_fill_col_count;

        if (count != 0) {
            uint8_t value = key_map[RGB_SPACEBAR_ROW * RGB_MATRIX_COLS + RGB_SPACEBAR_COL];

            for (uint32_t i = 0; i != count; ++i) {
                uint32_t fill_col = g_layout_spacebar_fill_cols[i];

                if (fill_col < RGB_MATRIX_COLS) {
                    key_map[RGB_SPACEBAR_ROW * RGB_MATRIX_COLS + fill_col] = value;
                }
            }
        }
    }
}

/*
 * Pseudo-random palette colour 1..7 for a key press, different from the previous one of the
 * layer. Stores the choice in *last_color and returns its three cell bits.
 */
static uint8_t rgb_effect_pick_press_color(uint8_t *last_color)
{
    uint32_t color;

    if ((g_clock_uptime_ms & 7) + 1 > 6) {
        color = (*last_color != 7) ? 7 : 1;
    } else {
        /* KNOWN ISSUE: the volatile tick counter is read a second time here instead of reusing
         * the first value. A tick between the two reads can yield 7, two ticks 8; 8 is stored as
         * last colour and gives cell colour bits 0 (kind 5: the cell is cleared, the key press
         * starts no ripple). Matches stock firmware 2.14.1. */
        uint32_t tick = g_clock_uptime_ms & 7;

        color = tick + 1;
        if (*last_color == color) {
            color = (*last_color != 7) ? tick + 2 : 1;
        }
    }

    *last_color = (uint8_t)color;
    return (uint8_t)(color & RGB_RIPPLE_CELL_COLOR_MASK);
}

/*
 * Feeds one key event (enum rgb_key_event) of the key at (row, col) into every layer that
 * reacts to keys: reactive fade and key ripple start on a press, the key test effect records
 * the press / actuation mark.
 */
void rgb_effects_handle_key_event(uint32_t row, uint32_t col, uint32_t event)
{
    uint32_t layer_count = g_rgb_effect_layer_count;

    for (uint8_t i = 0; i < layer_count; ++i) {
        rgb_effect_layer_t *layer = &g_rgb_effect_layers[i];

        switch (layer->cfg.which_effect) {
        case RgbEffectLayer_reactive_fade_tag:
            if (event == RGB_KEY_EVENT_PRESSED) {
                rgb_effect_reactive_state_t *reactive = &layer->state.reactive;

                if ((layer->cfg.effect.reactive_fade.flags & 1) == 0) {
                    reactive->cells[row][col] = RGB_REACTIVE_CELL_PRESSED;
                } else {
                    reactive->cells[row][col] =
                        (uint8_t)(rgb_effect_pick_press_color(&reactive->last_color) | RGB_REACTIVE_CELL_PALETTE_PRESSED);
                }
                rgb_effect_copy_spacebar_cell(&reactive->cells[0][0], row, col);
                reactive->active = 1;
                layer_count = g_rgb_effect_layer_count; /* re-read after the call */
            }
            break;

        case RgbEffectLayer_key_ripple_tag:
            if (event == RGB_KEY_EVENT_PRESSED) {
                rgb_effect_ripple_state_t *ripple = &layer->state.ripple;
                uint8_t *cell = &ripple->cells[row][col];
                uint8_t color;

                /* New ripple head: propagation code 0, tail bit clear. */
                *cell &= RGB_RIPPLE_CELL_COLOR_MASK;
                if ((layer->cfg.effect.key_ripple.flags & 1) == 0) {
                    color = 1;
                } else {
                    color = rgb_effect_pick_press_color(&ripple->last_color);
                }
                *cell = (uint8_t)((*cell & ~RGB_RIPPLE_CELL_COLOR_MASK) | color);
                ripple->active = 1;
            }
            break;

        case RgbEffectLayer_key_test_tag: {
            rgb_effect_key_map_state_t *key_map = &layer->state.key_map;
            uint8_t *mark = &key_map->cells[row][col];

            if (event == RGB_KEY_EVENT_ACTUATED) {
                *mark = RGB_KEY_TEST_ACTUATED;
            } else if (event == RGB_KEY_EVENT_RESET) {
                *mark = RGB_KEY_TEST_IDLE;
            } else if (event == RGB_KEY_EVENT_RELEASED) {
                if (*mark == RGB_KEY_TEST_PRESSED) {
                    *mark = RGB_KEY_TEST_ACTUATED;
                }
            } else {
                *mark = RGB_KEY_TEST_PRESSED;
            }
            key_map->dirty = 1;
            break;
        }

        default:
            break;
        }
    }
}

/*
 * Key travel 0..4095 through the gamepad dead zone and response curve; false when the key is
 * inside the dead zone (level 0), else *curve_out = curve output 0..1.
 */
static bool rgb_effect_travel_through_gamepad_curve(uint32_t travel, float *curve_out)
{
    float normalized;

    if (travel == 0) {
        return false;
    }
    normalized = hid_gamepad_normalize_travel(travel);
    if (normalized == 0.0f) {
        return false;
    }
    *curve_out = hid_gamepad_apply_response_curve(normalized);
    return true;
}

/* 8-bit effect level of a key (kinds 8, 9, 10, 13). */
static uint32_t rgb_effect_travel_to_level8(bool use_gamepad_curve, uint32_t usb_mode, uint32_t travel,
                                            uint32_t linear_level)
{
    float curve;

    if (!use_gamepad_curve || usb_mode == USB_MODE_NO_GAMEPAD) {
        return (linear_level > RGB_LEVEL_DEADZONE) ? linear_level : 0;
    }
    if (!rgb_effect_travel_through_gamepad_curve(travel, &curve)) {
        return 0;
    }
    return (uint8_t)(uint32_t)(curve * 255.0f);
}

/* 12-bit effect level of a key (kind 7). */
static uint32_t rgb_effect_travel_to_level12(bool use_gamepad_curve, uint32_t usb_mode, uint32_t travel)
{
    float curve;

    if (!use_gamepad_curve || usb_mode == USB_MODE_NO_GAMEPAD) {
        return (travel > RGB_LEVEL_METER_DEADZONE) ? travel : 0;
    }
    if (!rgb_effect_travel_through_gamepad_curve(travel, &curve)) {
        return 0;
    }
    if (!(curve > 0.0f)) { /* also catches NaN */
        return 0;
    }
    if (curve >= 1.0f) {
        return RGB_TRAVEL_FULL;
    }
    return (uint16_t)(uint32_t)(curve * 4095.0f);
}

/*
 * Feeds the analog travel (0..4095) of the key at (row, col) into every layer that shows key
 * depth: level meter (deepest key), analog spotlight / glow and kind 13 (per-key level map),
 * analog RGB fill (three keys of row 5). Sets the dirty flag of a layer whose state changed.
 */
void rgb_effects_feed_key_travel(uint8_t row, uint8_t col, uint16_t travel)
{
    uint32_t layer_count = g_rgb_effect_layer_count;
    uint32_t usb_mode;
    uint32_t linear_level;

    if (layer_count == 0) {
        return;
    }
    usb_mode = g_usb_link_flags & USB_LINK_MODE_MASK;
    linear_level = (uint8_t)(255u * travel / RGB_TRAVEL_FULL);

    for (uint32_t i = 0; i != layer_count; ++i) {
        rgb_effect_layer_t *layer = &g_rgb_effect_layers[i];

        switch (layer->cfg.which_effect) {
        case RgbEffectLayer_level_meter_tag: {
            rgb_effect_level_meter_state_t *meter = &layer->state.level_meter;
            uint32_t level =
                rgb_effect_travel_to_level12(layer->cfg.effect.level_meter.use_gamepad_curve, usb_mode, travel);

            if (meter->level < level) {
                /* A deeper key takes the meter over. */
                meter->row = row;
                meter->col = col;
                meter->level = (uint16_t)level;
                meter->dirty = 1;
            } else if (meter->row == row && meter->col == col && meter->level != level) {
                /* The owning key follows its travel, also back down to 0. */
                meter->level = (uint16_t)level;
                meter->dirty = 1;
            }
            break;
        }

        case RgbEffectLayer_analog_spotlight_tag:
        case RgbEffectLayer_analog_glow_tag:
        case RgbEffectLayer_f13_tag: {
            /* The flag is byte +5 of the layer entry for all three kinds. Kind 13 has a one-byte
             * parameter message, so for it this reads a byte nothing ever sets. */
            rgb_effect_key_map_state_t *key_map = &layer->state.key_map;
            uint32_t level = rgb_effect_travel_to_level8(layer->cfg.effect.analog_glow.use_gamepad_curve, usb_mode,
                                                         travel, linear_level);

            if (key_map->cells[row][col] != level) {
                key_map->cells[row][col] = (uint8_t)level;
                key_map->dirty = 1;
            }
            break;
        }

        case RgbEffectLayer_analog_rgb_fill_tag: {
            rgb_effect_fill_state_t *fill = &layer->state.fill;
            uint32_t level = rgb_effect_travel_to_level8(layer->cfg.effect.analog_rgb_fill.use_gamepad_curve,
                                                         usb_mode, travel, linear_level);

            if (row != RGB_FILL_ROW) {
                break;
            }
            if (col == RGB_FILL_RED_COL) {
                if (fill->red != level) {
                    fill->red = (uint8_t)level;
                    fill->dirty = 1;
                }
            } else if (col == RGB_FILL_GREEN_COL) {
                if (fill->green != level) {
                    fill->green = (uint8_t)level;
                    fill->dirty = 1;
                }
            } else if (col == RGB_FILL_BLUE_COL) {
                if (fill->blue != level) {
                    fill->blue = (uint8_t)level;
                    fill->dirty = 1;
                }
            }
            break;
        }

        default:
            break;
        }
    }
}
