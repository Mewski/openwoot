/*
 * rgb_effects_state.c - run-time state of the effect layers (g_rgb_effect_layers): initial
 * state per effect kind, take-over of the layer list of the rgb settings, reset of the
 * key-driven state, and the lazily computed polar tables of the polar gradient effect.
 */
#include "rgb/rgb_effects.h"

#include <math.h>
#include <string.h>

/* Size of the per-key cell map at the start of the ripple / reactive / key map states. */
#define RGB_EFFECT_CELL_MAP_SIZE (RGB_MATRIX_ROWS * RGB_MATRIX_COLS)

/* Scanner window limits when the width parameter changes while the effect runs. */
#define RGB_SCANNER_WIDTH_MAX_COLS RGB_MATRIX_COLS
#define RGB_SCANNER_WIDTH_MAX_ROWS RGB_MATRIX_ROWS

/* Centre of the key matrix used by the polar tables. */
#define RGB_POLAR_CENTER_ROW ((float)RGB_MATRIX_ROWS / 2.0f)
#define RGB_POLAR_CENTER_COL ((float)RGB_MATRIX_COLS / 2.0f)
#define RGB_POLAR_TWO_PI     6.2831855f

/*
 * Clears the key-depth state of all layers (level meter, analog spotlight, analog glow,
 * analog RGB fill), as if every key were released.
 */
void rgb_effects_reset_key_state(void)
{
    uint32_t layer_count = g_rgb_effect_layer_count;

    for (uint32_t i = 0; i != layer_count; ++i) {
        rgb_effect_layer_t *layer = &g_rgb_effect_layers[i];

        switch (layer->cfg.which_effect) {
        case RgbEffectLayer_level_meter_tag:
            layer->state.level_meter.level = 0;
            layer->state.level_meter.dirty = 1;
            break;

        case RgbEffectLayer_analog_spotlight_tag: {
            rgb_effect_key_map_state_t *key_map = &layer->state.key_map;
            bool changed = false;

            for (uint32_t row = 0; row != RGB_MATRIX_ROWS; ++row) {
                for (uint32_t col = 0; col != RGB_MATRIX_COLS; ++col) {
                    if (key_map->cells[row][col] != 0) {
                        key_map->cells[row][col] = 0;
                        changed = true;
                    }
                }
            }
            if (changed) {
                key_map->dirty = 1;
            }
            break;
        }

        case RgbEffectLayer_analog_glow_tag:
            memset(layer->state.key_map.cells, 0, RGB_EFFECT_CELL_MAP_SIZE);
            layer->state.key_map.dirty = 1;
            break;

        case RgbEffectLayer_analog_rgb_fill_tag:
            layer->state.fill.red = 0;
            layer->state.fill.green = 0;
            layer->state.fill.blue = 0;
            break;

        default:
            break;
        }
    }
}

/*
 * Computes g_rgb_key_distance_table and g_rgb_key_angle_table (distance and angle of every
 * key position from the matrix centre) on the first call; later calls return at once.
 */
void rgb_init_polar_tables(void)
{
    if (g_rgb_polar_tables_ready != 0) {
        return;
    }

    for (int32_t row = 0; row != RGB_MATRIX_ROWS; ++row) {
        float dy = RGB_POLAR_CENTER_ROW - (float)row;

        for (int32_t col = 0; col != RGB_MATRIX_COLS; ++col) {
            float dx = RGB_POLAR_CENTER_COL - (float)col;
            float distance = sqrtf(dy * dy + dx * dx);
            float angle = atan2f(dy, dx);

            g_rgb_key_distance_table[row][col] = (uint8_t)(uint32_t)(distance * 16.0f);
            g_rgb_key_angle_table[row][col] = (uint8_t)(uint32_t)((angle / RGB_POLAR_TWO_PI + 0.5f) * 255.0f);
        }
    }
    g_rgb_polar_tables_ready = 1;
}

/* Hue range of the rainbow wave and hue cycle effects for a preset. */
static void rgb_effect_set_hue_range(rgb_effect_hue_state_t *hue, uint32_t hue_preset)
{
    switch (hue_preset) {
    case RgbHuePreset_RGB_HUE_PRESET_110_TO_184:
        hue->hue_a = 110;
        hue->hue_b = 184;
        break;
    case RgbHuePreset_RGB_HUE_PRESET_2_TO_39:
        hue->hue_a = 2;
        hue->hue_b = 39;
        break;
    case RgbHuePreset_RGB_HUE_PRESET_64_TO_114:
        hue->hue_a = 64;
        hue->hue_b = 114;
        break;
    case RgbHuePreset_RGB_HUE_PRESET_214_TO_254:
        hue->hue_a = 214;
        hue->hue_b = 254;
        break;
    case RgbHuePreset_RGB_HUE_PRESET_113_TO_204:
        hue->hue_a = 113;
        hue->hue_b = 204;
        break;
    default:
        hue->hue_a = 0;
        hue->hue_b = 255;
        break;
    }
}

/*
 * Initial animation state of a layer for the effect kind and parameters in slot->cfg. The
 * caller has zeroed slot->state before.
 */
void rgb_effect_reset_slot_state(rgb_effect_layer_t *slot)
{
    switch (slot->cfg.which_effect) {
    case RgbEffectLayer_rainbow_wave_tag:
        /* The phase is left alone (zero after the memset of the caller). */
        rgb_effect_set_hue_range(&slot->state.hue, slot->cfg.effect.rainbow_wave.hue_preset);
        break;

    case RgbEffectLayer_scanner_tag:
        slot->state.scanner.window_end = (int8_t)slot->cfg.effect.scanner.width;
        slot->state.scanner.window_start = 0;
        slot->state.scanner.phase = slot->cfg.effect.scanner.start_direction;
        slot->state.scanner.timer_ms = 0;
        break;

    case RgbEffectLayer_breathing_tag:
        slot->state.breathing.level = 0;
        slot->state.breathing.falling = 0;
        break;

    case RgbEffectLayer_hue_cycle_tag:
        rgb_effect_set_hue_range(&slot->state.hue, slot->cfg.effect.hue_cycle.hue_preset);
        slot->state.hue.phase = 0;
        break;

    case RgbEffectLayer_key_ripple_tag:
        memset(slot->state.ripple.cells, 0, RGB_EFFECT_CELL_MAP_SIZE);
        slot->state.ripple.active = 1;
        slot->state.ripple.move_timer_ms = 0;
        slot->state.ripple.decay_timer_ms = 0;
        break;

    case RgbEffectLayer_reactive_fade_tag:
        memset(slot->state.reactive.cells, 0, RGB_EFFECT_CELL_MAP_SIZE);
        slot->state.reactive.active = 1;
        break;

    case RgbEffectLayer_level_meter_tag:
        slot->state.level_meter.row = 0xFF;
        slot->state.level_meter.col = 0xFF;
        slot->state.level_meter.dirty = 1;
        slot->state.level_meter.level = 0;
        break;

    case RgbEffectLayer_analog_spotlight_tag:
    case RgbEffectLayer_analog_glow_tag:
    case RgbEffectLayer_key_test_tag:
        memset(slot->state.key_map.cells, 0, RGB_EFFECT_CELL_MAP_SIZE);
        slot->state.key_map.dirty = 1;
        break;

    case RgbEffectLayer_analog_rgb_fill_tag:
        slot->state.fill.red = 0;
        slot->state.fill.green = 0;
        slot->state.fill.blue = 0;
        slot->state.fill.dirty = 1;
        break;

    case RgbEffectLayer_simplex_noise_tag:
        slot->state.noise.time = 0;
        break;

    case RgbEffectLayer_polar_gradient_tag:
        slot->state.polar.phase = 0;
        rgb_init_polar_tables();
        break;

    default: /* kinds 12, 13 and unknown kinds have no state */
        break;
    }
}

/*
 * Takes the changed parameters of a layer over whose kind stays the same, and resets only the
 * state that depends on them, so that a running animation keeps going. `slot->cfg` still holds
 * the old parameters; the caller copies the whole entry afterwards.
 */
static void rgb_effect_update_slot_params(rgb_effect_layer_t *slot, const RgbEffectLayer *entry)
{
    switch (entry->which_effect) {
    case RgbEffectLayer_rainbow_wave_tag:
        if (slot->cfg.effect.rainbow_wave.hue_preset != entry->effect.rainbow_wave.hue_preset) {
            rgb_effect_set_hue_range(&slot->state.hue, entry->effect.rainbow_wave.hue_preset);
        }
        break;

    case RgbEffectLayer_scanner_tag: {
        uint32_t width = entry->effect.scanner.width;

        if (slot->cfg.effect.scanner.start_direction != entry->effect.scanner.start_direction) {
            slot->state.scanner.phase = entry->effect.scanner.start_direction;
            slot->state.scanner.window_end = (int8_t)slot->cfg.effect.scanner.width; /* the old width */
            slot->state.scanner.window_start = 0;
        }
        if (slot->cfg.effect.scanner.width != width) {
            if (slot->state.scanner.phase <= RgbScannerDirection_RGB_SCANNER_DIRECTION_RIGHT) {
                if (width >= RGB_SCANNER_WIDTH_MAX_COLS) {
                    width = RGB_SCANNER_WIDTH_MAX_COLS;
                }
            } else if (width >= RGB_SCANNER_WIDTH_MAX_ROWS) {
                width = RGB_SCANNER_WIDTH_MAX_ROWS;
            }
            slot->state.scanner.window_end = (int8_t)width;
            slot->state.scanner.window_start = 0;
        }
        break;
    }

    case RgbEffectLayer_breathing_tag:
        break;

    case RgbEffectLayer_hue_cycle_tag:
        rgb_effect_set_hue_range(&slot->state.hue, entry->effect.hue_cycle.hue_preset);
        slot->state.hue.phase = 0;
        break;

    case RgbEffectLayer_key_ripple_tag:
        memset(slot->state.ripple.cells, 0, RGB_EFFECT_CELL_MAP_SIZE);
        slot->state.ripple.active = 1;
        slot->state.ripple.move_timer_ms = 0;
        slot->state.ripple.decay_timer_ms = 0;
        break;

    case RgbEffectLayer_reactive_fade_tag:
        memset(slot->state.reactive.cells, 0, RGB_EFFECT_CELL_MAP_SIZE);
        slot->state.reactive.active = 1;
        break;

    case RgbEffectLayer_level_meter_tag:
        slot->state.level_meter.level = 0;
        slot->state.level_meter.row = 0xFF;
        slot->state.level_meter.col = 0xFF;
        slot->state.level_meter.dirty = 1;
        break;

    case RgbEffectLayer_analog_spotlight_tag:
    case RgbEffectLayer_analog_glow_tag:
    case RgbEffectLayer_key_test_tag:
        memset(slot->state.key_map.cells, 0, RGB_EFFECT_CELL_MAP_SIZE);
        slot->state.key_map.dirty = 1;
        break;

    case RgbEffectLayer_analog_rgb_fill_tag:
        slot->state.fill.red = 0;
        slot->state.fill.green = 0;
        slot->state.fill.blue = 0;
        slot->state.fill.dirty = 1;
        break;

    case RgbEffectLayer_simplex_noise_tag:
        slot->state.noise.time = 0;
        break;

    case RgbEffectLayer_polar_gradient_tag:
        slot->state.polar.phase = 0;
        rgb_init_polar_tables();
        break;

    default: /* kinds 12, 13 and unknown kinds */
        break;
    }
}

/*
 * Takes the layer list of g_rgb_settings over into g_rgb_effect_layers (at most six layers).
 * A layer whose kind is unchanged keeps its animation state as far as the new parameters
 * allow; a new layer or a layer of another kind starts from its initial state.
 */
void rgb_effects_apply_layer_settings(void)
{
    const RgbEffectLayerList *list = &g_rgb_settings.effect.layers;
    uint8_t index = 0;

    if (list->layers_count != 0) {
        uint32_t limit;

        do {
            const RgbEffectLayer *entry = &list->layers[index];
            rgb_effect_layer_t *slot = &g_rgb_effect_layers[index];

            if (g_rgb_effect_layer_count > index && entry->which_effect == slot->cfg.which_effect) {
                rgb_effect_update_slot_params(slot, entry);
                memcpy(&slot->cfg, entry, sizeof(slot->cfg));
            } else {
                /* New layer or another kind: start from scratch. */
                memcpy(&slot->cfg, entry, sizeof(slot->cfg));
                memset(&slot->state, 0, sizeof(slot->state));
                rgb_effect_reset_slot_state(slot);
            }

            ++index;
            limit = list->layers_count;
            if (limit >= RGB_EFFECT_LAYER_MAX) {
                limit = RGB_EFFECT_LAYER_MAX;
            }
        } while (index < limit);
    }

    g_rgb_effect_layer_count = index;
}
