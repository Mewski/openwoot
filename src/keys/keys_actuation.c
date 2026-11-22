/*
 * keys_actuation.c - actuation points and the digital press decision.
 *
 * Per-key actuation points of profile file "act" (apply / sanitise), the distance encoding
 * helper, the effective actuation threshold of a key and the press / release decision with
 * fixed actuation or rapid trigger.
 */
#include "keys/keys.h"
#include "analog/analog.h"
#include "analog/analog_data.h"
#include "layout/layout.h"
#include "rgb/rgb_types.h"
#include "rgb/rgb_data.h"
#include "rgb/rgb_effects.h"

/* Defaults written by the sanitisers when a stored value is unusable. */
#define KEYS_ACTUATION_DEFAULT_U8    95u     /* legacy 8-bit default: 1.5 mm of 4 mm */
#define KEYS_ACTUATION_DEFAULT_Q12   0x605u  /* the same in 12-bit travel units */

#define KEYS_TRAVEL_MAX              4095u   /* full travel, 12 bit */
#define KEYS_DISTANCE_U8_MAX         0xFFu   /* full travel in the legacy 8-bit encoding */

/* Key id of a sparse list item -> scan position, ANALOG_NO_SCAN_POS when the id is out of range
 * or the position has no key. */
static uint8_t keys_key_id_to_scan_pos(uint8_t key_id)
{
    uint8_t col = KEYS_KEY_ID_COL(key_id);

    if (key_id > KEYS_KEY_ID_MAX || col > LAYOUT_COLS - 1u) {
        return ANALOG_NO_SCAN_POS;
    }
    return g_layout_keyid_to_scanpos[LAYOUT_COLS * KEYS_KEY_ID_ROW(key_id) + col];
}

/* Applies profile file "act": resets the per-key actuation point of every key record to "use
 * the profile-wide value", then stores the non-zero values of the message. Four formats: a
 * matrix (one value per row / column) or a sparse list (key id in the low byte, value above
 * it), each with legacy 8-bit values or 16-bit values in the distance encoding. The number of
 * non-zero entries goes to g_keys_per_key_actuation_count, also for entries without key. */
void keys_apply_actuation_map(const ActuationPoints *points)
{
    uint8_t override_count = 0;
    bool any_override = false;

    g_keys_per_key_actuation_count = 0;
    for (uint32_t pos = 0; pos != ANALOG_KEY_COUNT; pos++) {
        g_analog_key_states[pos].actuation_point = KEYS_DISTANCE_UNSET;
    }

    switch (points->which_map) {
    case ActuationPoints_matrix_u8_tag:
        for (uint32_t row = 0; row != LAYOUT_ROWS; row++) {
            for (uint32_t col = 0; col != LAYOUT_KEYMAP_COLS; col++) {
                uint8_t value = points->map.matrix_u8.rows[row].values[col];

                if (value == 0) {
                    continue;
                }
                if (col <= LAYOUT_COLS - 1u) {
                    uint8_t pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];

                    if (pos != ANALOG_NO_SCAN_POS) {
                        g_analog_key_states[pos].actuation_point = value;
                    }
                }
                override_count++;
                any_override = true;
            }
        }
        break;

    case ActuationPoints_list_u8_tag: {
        uint32_t item_count = points->map.list_u8.items_count;

        /* The index is 8 bit wide. */
        for (uint8_t i = 0; i < item_count; i++) {
            uint16_t item = points->map.list_u8.items[i];
            uint8_t key_id = (uint8_t)item;
            uint8_t value = (uint8_t)(item >> 8);

            if (value == 0) {
                continue;
            }
            override_count++;
            uint8_t pos = keys_key_id_to_scan_pos(key_id);
            if (pos != ANALOG_NO_SCAN_POS) {
                g_analog_key_states[pos].actuation_point = value;
            }
            any_override = true;
        }
        break;
    }

    case ActuationPoints_matrix_u16_tag:
        for (uint32_t row = 0; row != LAYOUT_ROWS; row++) {
            for (uint32_t col = 0; col != LAYOUT_KEYMAP_COLS; col++) {
                uint16_t value = points->map.matrix_u16.rows[row].values[col];

                if (value == 0) {
                    continue;
                }
                if (col <= LAYOUT_COLS - 1u) {
                    uint8_t pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];

                    if (pos != ANALOG_NO_SCAN_POS) {
                        g_analog_key_states[pos].actuation_point = value;
                    }
                }
                override_count++;
                any_override = true;
            }
        }
        break;

    case ActuationPoints_list_u16_tag: {
        uint32_t item_count = points->map.list_u16.items_count;

        for (uint8_t i = 0; i < item_count; i++) {
            uint32_t item = points->map.list_u16.items[i];
            uint8_t key_id = (uint8_t)item;
            uint16_t value = (uint16_t)(item >> 16);

            if (value == 0) {
                continue;
            }
            override_count++;
            uint8_t pos = keys_key_id_to_scan_pos(key_id);
            if (pos != ANALOG_NO_SCAN_POS) {
                g_analog_key_states[pos].actuation_point = value;
            }
            any_override = true;
        }
        break;
    }

    default:
        return;
    }

    if (any_override) {
        g_keys_per_key_actuation_count = override_count;
    }
}

/* Brings a stored distance into the 14-bit form of the distance encoding. `min_q12` and
 * `default_q12` are 12-bit travel values. A legacy 8-bit value is left as it is when it is
 * usable and not below the minimum. A value that has neither encoding, or whose 14-bit fraction
 * is 4 or less, is replaced by the default; a usable value below the minimum is raised to it. */
void keys_sanitize_distance(uint16_t *value, uint32_t min_q12, uint32_t default_q12)
{
    uint32_t stored = *value;
    uint32_t q14;
    bool usable;

    if (stored > KEYS_DISTANCE_LEGACY_MAX) {
        q14 = stored & KEYS_DISTANCE_Q14_MASK;
        usable = KEYS_DISTANCE_IS_Q14(stored) && q14 > 4u;
    } else {
        q14 = (uint16_t)(KEYS_DISTANCE_Q14_MASK * stored / KEYS_DISTANCE_U8_MAX);
        usable = q14 > 4u;
    }

    if (!usable) {
        *value = (uint16_t)((KEYS_DISTANCE_Q14_MASK * default_q12 / KEYS_TRAVEL_MAX) | KEYS_DISTANCE_Q14_FLAG);
        return;
    }

    uint32_t min_q14 = (uint16_t)(KEYS_DISTANCE_Q14_MASK * min_q12 / KEYS_TRAVEL_MAX);
    if (q14 < min_q14) {
        *value = (uint16_t)(min_q14 | KEYS_DISTANCE_Q14_FLAG);
    }
}

/* Legacy 8-bit actuation point of the key at row / col: 0 becomes the default, values below the
 * key's minimum (0.1 mm) are raised to it. */
static void keys_sanitize_actuation_u8(uint8_t *value, uint32_t row, uint32_t col)
{
    uint8_t min_u8 = (uint8_t)(KEYS_DISTANCE_U8_MAX * analog_get_key_travel_per_0p1mm(row, col) / KEYS_TRAVEL_MAX);

    if (*value != 0) {
        if (*value < min_u8) {
            *value = min_u8;
        }
    } else {
        *value = KEYS_ACTUATION_DEFAULT_U8;
    }
}

/* Sanitises an "act" message before it is stored or applied: every non-zero value of a
 * position that carries a key gets the default (1.5 mm) when it is unusable and is raised to
 * the key's minimum (0.1 mm of its switch travel). Entries without key are left untouched. */
void keys_sanitize_actuation_map(ActuationPoints *points)
{
    switch (points->which_map) {
    case ActuationPoints_matrix_u8_tag:
        for (uint32_t row = 0; row != LAYOUT_ROWS; row++) {
            for (uint32_t col = 0; col != LAYOUT_COLS; col++) {
                uint8_t *value = &points->map.matrix_u8.rows[row].values[col];

                if (g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col] > ANALOG_KEY_COUNT - 1u || *value == 0) {
                    continue;
                }
                keys_sanitize_actuation_u8(value, (uint8_t)row, (uint8_t)col);
            }
        }
        break;

    case ActuationPoints_list_u8_tag:
        /* The count is read again after every item, the index is 8 bit wide. */
        for (uint8_t i = 0; i < points->map.list_u8.items_count; i++) {
            uint16_t item = points->map.list_u8.items[i];
            uint8_t key_id = (uint8_t)item;
            uint8_t value = (uint8_t)(item >> 8);

            if (keys_key_id_to_scan_pos(key_id) > ANALOG_KEY_COUNT - 1u) {
                continue;
            }
            /* Unlike the matrix format, a list item with value 0 gets the default. */
            keys_sanitize_actuation_u8(&value, KEYS_KEY_ID_ROW(key_id), KEYS_KEY_ID_COL(key_id));
            points->map.list_u8.items[i] = (uint16_t)(key_id | (value << 8));
        }
        break;

    case ActuationPoints_matrix_u16_tag:
        for (uint32_t row = 0; row != LAYOUT_ROWS; row++) {
            for (uint32_t col = 0; col != LAYOUT_COLS; col++) {
                uint16_t *value = &points->map.matrix_u16.rows[row].values[col];

                if (g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col] <= ANALOG_KEY_COUNT - 1u && *value != 0) {
                    keys_sanitize_distance(value, analog_get_key_travel_per_0p1mm((uint8_t)row, (uint8_t)col),
                                           KEYS_ACTUATION_DEFAULT_Q12);
                }
            }
        }
        break;

    case ActuationPoints_list_u16_tag:
        for (uint8_t i = 0; i < points->map.list_u16.items_count; i++) {
            uint32_t item = points->map.list_u16.items[i];
            uint8_t key_id = (uint8_t)item;
            uint16_t value = (uint16_t)(item >> 16);

            if (keys_key_id_to_scan_pos(key_id) > ANALOG_KEY_COUNT - 1u) {
                continue;
            }
            keys_sanitize_distance(&value,
                                   analog_get_key_travel_per_0p1mm(KEYS_KEY_ID_ROW(key_id), KEYS_KEY_ID_COL(key_id)),
                                   KEYS_ACTUATION_DEFAULT_Q12);
            points->map.list_u16.items[i] = (item & 0x0000FFFFu) | ((uint32_t)value << 16);
        }
        break;

    default:
        break;
    }
}

/* Distance encoding -> 12-bit travel; 0 for values that have neither encoding. */
static uint32_t keys_distance_to_travel(uint32_t distance)
{
    if (distance <= KEYS_DISTANCE_LEGACY_MAX) {
        return KEYS_TRAVEL_MAX * distance / KEYS_DISTANCE_U8_MAX;
    }
    if (KEYS_DISTANCE_IS_Q14(distance)) {
        return KEYS_TRAVEL_MAX * (distance & KEYS_DISTANCE_Q14_MASK) / KEYS_DISTANCE_Q14_MASK;
    }
    return 0;
}

/* Effective actuation point of a key in 12-bit travel units: the per-key value, or the
 * profile-wide value when the key has none, decoded from the distance encoding and never below
 * 0.1 mm of the key's switch travel. A value with neither encoding gives that minimum. */
uint32_t keys_get_actuation_threshold(keys_key_ref_t *key_ctx)
{
    uint32_t actuation_point = key_ctx->key_state->actuation_point;
    uint32_t min_travel;
    uint32_t travel;

    if (actuation_point == KEYS_DISTANCE_UNSET) {
        actuation_point = g_keys_actuation_point;
    }

    if (actuation_point <= KEYS_DISTANCE_LEGACY_MAX) {
        min_travel = analog_get_key_travel_per_0p1mm(KEYS_KEY_ID_ROW(key_ctx->key_id), KEYS_KEY_ID_COL(key_ctx->key_id));
        travel = (uint16_t)(KEYS_TRAVEL_MAX * actuation_point / KEYS_DISTANCE_U8_MAX);
        if (min_travel < travel) {
            min_travel = travel;
        }
        return (uint16_t)min_travel;
    }

    if (KEYS_DISTANCE_IS_Q14(actuation_point)) {
        min_travel = analog_get_key_travel_per_0p1mm(KEYS_KEY_ID_ROW(key_ctx->key_id), KEYS_KEY_ID_COL(key_ctx->key_id));
        travel = (uint16_t)(KEYS_TRAVEL_MAX * (actuation_point & KEYS_DISTANCE_Q14_MASK) / KEYS_DISTANCE_Q14_MASK);
        if (min_travel < travel) {
            return travel;
        }
        return min_travel;
    }

    return analog_get_key_travel_per_0p1mm(KEYS_KEY_ID_ROW(key_ctx->key_id), KEYS_KEY_ID_COL(key_ctx->key_id));
}

/* max(0, 96 - max(0, distance)): the hysteresis left after the key moved `distance` past the
 * actuation point; negative results mean "none". The arithmetic is 16 bit wide. */
static int32_t keys_remaining_hysteresis(uint32_t distance)
{
    int16_t past = (int16_t)distance;

    if (past < 0) {
        past = 0;
    }
    return (int16_t)(KEYS_PRESS_HYSTERESIS - past);
}

/* Fixed actuation point with a hysteresis that shrinks the further the key moved past it. */
static void keys_update_press_state_fixed(keys_key_ref_t *key_ctx, uint32_t travel, uint8_t flags)
{
    analog_key_state_t *key = key_ctx->key_state;
    int32_t threshold = (int32_t)keys_get_actuation_threshold(key_ctx);
    uint32_t ref_travel = key->press_ref_travel;
    int32_t hysteresis;

    if ((flags & ANALOG_KEY_FLAG_PRESSED) != 0) {
        /* Pressed: press_ref_travel tracks the deepest point. */
        bool deeper = travel > ref_travel;

        if (deeper) {
            ref_travel = (uint16_t)travel;
        }
        hysteresis = keys_remaining_hysteresis(ref_travel - (uint32_t)threshold);
        threshold = (int16_t)threshold;
        if (deeper) {
            key->press_ref_travel = (uint16_t)travel;
        }
        if (hysteresis >= 0) {
            threshold -= hysteresis;
        }
        if (threshold < (int32_t)KEYS_RELEASE_FLOOR) {
            threshold = KEYS_RELEASE_FLOOR;
        }
        if (travel < (uint16_t)threshold) {
            key->press_ref_travel = (uint16_t)travel;
            key->flags = (key->flags & ~(ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED)) | ANALOG_KEY_FLAG_CHANGED;
        }
        return;
    }

    /* Released: press_ref_travel tracks the highest point. */
    if (travel < ref_travel) {
        ref_travel = travel;
        key->press_ref_travel = (uint16_t)travel;
    }
    if (ref_travel != 0) {
        hysteresis = keys_remaining_hysteresis((uint32_t)threshold - ref_travel);
        threshold = (int16_t)threshold;
        if (hysteresis >= 0) {
            threshold += hysteresis;
        }
        if (threshold >= (int32_t)KEYS_TRAVEL_MAX) {
            threshold = KEYS_TRAVEL_MAX;
        }
        threshold = (uint16_t)threshold;
    }
    if (travel < (uint32_t)threshold) {
        return;
    }

    key->press_ref_travel = (uint16_t)travel;
    key->flags = (key->flags & ~(ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED))
                 | ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED;
}

/* Rapid trigger: press / release follow the direction changes of the travel while the key is
 * inside the rapid-trigger zone. */
static void keys_update_press_state_rapid_trigger(keys_key_ref_t *key_ctx, uint32_t travel, uint8_t flags)
{
    analog_key_state_t *key = key_ctx->key_state;
    const RapidTriggerParams *params = key->rt_params;
    uint32_t threshold;
    bool left_zone;

    if (params == NULL) {
        params = &g_keys_rapid_trigger_params;
    }
    threshold = keys_get_actuation_threshold(key_ctx);

    if (params->continuous) {
        left_zone = travel == 0;
    } else {
        int32_t exit_travel = (int16_t)threshold - (int32_t)KEYS_PRESS_HYSTERESIS;

        if (exit_travel < (int32_t)KEYS_RELEASE_FLOOR) {
            exit_travel = KEYS_RELEASE_FLOOR;
        }
        left_zone = travel < (uint16_t)exit_travel;
    }

    if (left_zone) {
        if ((flags & ANALOG_KEY_FLAG_RT_ENGAGED) != 0) {
            key->flags &= (uint8_t)~ANALOG_KEY_FLAG_RT_ENGAGED;
            if (!g_rgb_sdk_mode_active) {
                rgb_effects_handle_key_event(KEYS_KEY_ID_ROW(key_ctx->key_id), KEYS_KEY_ID_COL(key_ctx->key_id),
                                             RGB_KEY_EVENT_RESET);
            }
            flags = key->flags;
        }
        if ((flags & ANALOG_KEY_FLAG_PRESSED) != 0) {
            key->flags = (key->flags & ~(ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED)) | ANALOG_KEY_FLAG_CHANGED;
        }
        return;
    }

    if ((flags & ANALOG_KEY_FLAG_RT_ENGAGED) == 0) {
        /* Entering the zone: the first press happens at the actuation point. */
        if ((flags & ANALOG_KEY_FLAG_PRESSED) == 0 && travel >= threshold) {
            key->press_ref_travel = (uint16_t)travel;
            key->flags = (flags & ~(ANALOG_KEY_FLAG_RT_ENGAGED | ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED))
                         | ANALOG_KEY_FLAG_RT_ENGAGED | ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED;
            if (!g_rgb_sdk_mode_active) {
                rgb_effects_handle_key_event(KEYS_KEY_ID_ROW(key_ctx->key_id), KEYS_KEY_ID_COL(key_ctx->key_id),
                                             RGB_KEY_EVENT_ACTUATED);
            }
        }
        return;
    }

    /* Positive: the key moved up from the reference point, negative: further down. */
    int32_t moved_up = (int16_t)(key->press_ref_travel - travel);

    if ((flags & ANALOG_KEY_FLAG_PRESSED) == 0) {
        uint32_t press_sensitivity = keys_distance_to_travel(params->sensitivity);

        if (moved_up >= -(int32_t)press_sensitivity && travel != KEYS_TRAVEL_MAX) {
            if (moved_up > (int32_t)KEYS_RT_FOLLOW_STEP) {
                key->press_ref_travel = (uint16_t)travel;
            }
            return;
        }
        key->press_ref_travel = (uint16_t)travel;
        key->flags = (key->flags & ~(ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED))
                     | ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED;
        return;
    }

    uint32_t release_distance = params->has_release_sensitivity ? params->release_sensitivity : params->sensitivity;
    int32_t release_sensitivity = (int32_t)keys_distance_to_travel(release_distance);

    if (moved_up > release_sensitivity) {
        key->flags = (key->flags & ~(ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED)) | ANALOG_KEY_FLAG_CHANGED;
        return;
    }
    if (moved_up < -(int32_t)KEYS_RT_FOLLOW_STEP) {
        key->press_ref_travel = (uint16_t)travel;
    }
}

/* Updates the pressed state of a key from its current 12-bit travel. Rapid trigger is used when
 * the key's per-key override says so, or, without override, when the profile-wide switch is
 * on; otherwise the fixed actuation point decides. A change sets ANALOG_KEY_FLAG_CHANGED
 * together with the new ANALOG_KEY_FLAG_PRESSED state. */
void keys_update_press_state(keys_key_ref_t *key_ctx, uint32_t travel)
{
    uint8_t flags = key_ctx->key_state->flags;
    bool rapid_trigger;

    if ((flags & ANALOG_KEY_FLAG_RT_OVERRIDE) != 0) {
        rapid_trigger = (flags & ANALOG_KEY_FLAG_RT_ON) != 0;
    } else {
        rapid_trigger = g_keys_rapid_trigger_enabled != 0;
    }

    if (rapid_trigger) {
        keys_update_press_state_rapid_trigger(key_ctx, travel, flags);
    } else {
        keys_update_press_state_fixed(key_ctx, travel, flags);
    }
}
