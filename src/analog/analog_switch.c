/*
 * analog_switch.c - hall switch profiles (file "sw"), run-time switch parameters and the
 * conversion of a sensor value into key travel through the per-switch travel curves.
 */
#include <string.h>

#include "analog/analog.h"
#include "adc_scan/adc_scan_data.h"
#include "layout/layout_data.h"
#include "profile/edition/edition.h"
#include "storage/storage.h"
#include "storage/storage_data.h"
#include "tables/analog_tables.h"

#define ANALOG_CURVE_LAST_INDEX   4095u  /* last entry of one travel curve */
#define ANALOG_TRAVEL_PER_0P1MM_DEFAULT 112u /* answer for positions without key */

/* Read a 16-bit table at a Q8 index: entry index_q8 >> 8, linearly interpolated towards the next
 * entry with the low byte as fraction (rounded). */
uint16_t analog_lut_interpolate(const uint16_t *lut, uint32_t index_q8)
{
    uint32_t index = (uint16_t)(index_q8 >> 8);
    uint32_t fraction = (uint8_t)index_q8;

    if (fraction == 0u) {
        return lut[index];
    }
    return (uint16_t)(lut[index] + ((uint32_t)((lut[index + 1u] - lut[index]) * fraction + 128u) >> 8));
}

/* Give the keyboard-wide description and every key group of `sw` that has no switch type the
 * default switch type, without travel offset. */
void analog_fill_missing_switch_types(SwitchProfiles *sw)
{
    if (!sw->has_default_params || sw->default_params.which_type == 0u) {
        sw->has_default_params = true;
        sw->default_params.which_type = 1u;
        sw->default_params.type.switch_type = (SwitchType)profile_edition_default_switch_type();
        sw->default_params.has_travel_offset = false;
    }

    for (uint8_t i = 0u; i < sw->entries_count; i++) {
        SwitchProfileEntry *entry = &sw->entries[i];
        SwitchKeyGroup *group = &entry->kind.key_group;

        if (entry->which_kind == 1u && (!group->has_params || group->params.which_type == 0u)) {
            group->has_params = true;
            group->params.which_type = 1u;
            group->params.type.switch_type = (SwitchType)profile_edition_default_switch_type();
            group->params.has_travel_offset = false;
        }
    }
}

/* Build the run-time switch parameters from a switch description: copy of the description,
 * bottom-out learning strength and nominal bottom-out level of the switch type, and the position
 * in the travel curve that counts as end of travel (shortened when a travel offset is set). */
void analog_build_switch_params(analog_switch_params_t *out_params, const SwitchParams *sw_params)
{
    uint32_t switch_type;
    uint32_t strength;

    out_params->sw = *sw_params;
    switch_type = sw_params->type.switch_type; /* not range-checked: the tables have 49 entries */
    out_params->curve_end_q8 = ANALOG_CURVE_END_Q8_MAX;
    strength = k_analog_switch_bottom_out_strength[switch_type];
    out_params->learn_strength = (uint8_t)strength;

    if (sw_params->has_travel_offset) {
        uint32_t travel = k_analog_switch_travel[switch_type];
        uint32_t remaining = (uint8_t)(travel - sw_params->travel_offset);
        const uint16_t *curve = k_analog_travel_curves[k_analog_switch_curve_index[switch_type]];
        uint32_t target;
        uint32_t low = 0u;
        uint32_t high = ANALOG_CURVE_LAST_INDEX;
        uint32_t mid = 0u;
        uint32_t end_q8;
        bool found = false;

        out_params->learn_strength = (uint8_t)(strength * remaining / travel);
        target = (uint16_t)(0x3FFFu * remaining / travel);

        /* Binary search of the shortened travel in the curve. The bounds are unsigned: a curve
         * whose first entry is above the target would run off the table (every curve starts at
         * 0, so this cannot happen). */
        do {
            mid = low + ((high - low) >> 1);
            if (curve[mid] < target) {
                low = mid + 1u;
            } else if (curve[mid] > target) {
                high = mid - 1u;
            } else {
                found = true;
                break;
            }
        } while (low <= high);

        if (found) {
            /* Exact hit: take the plateau of equal values around it. */
            uint32_t first = mid;
            uint32_t last;
            uint32_t next = mid;

            while (first != 0u && curve[first - 1u] == target) {
                first--;
            }
            do {
                last = next;
                next++;
            } while (next < 0x1000u && curve[next] == target);

            end_q8 = (first + (last - first) * target / 0x3FFFu) << 8;
            if (end_q8 >= ANALOG_CURVE_END_Q8_MAX) {
                end_q8 = ANALOG_CURVE_END_Q8_MAX;
            }
        } else if (low == 0u) {
            end_q8 = 0u;
        } else if (low >= 0x1000u) {
            end_q8 = ANALOG_CURVE_END_Q8_MAX;
        } else {
            /* Between curve[high] and curve[low] (low == high + 1): linear interpolation. */
            uint32_t step = (uint16_t)(curve[low] - curve[high]);

            end_q8 = ((step >> 1) + ((target - curve[high]) << 8)) / step + (high << 8);
            if (end_q8 >= ANALOG_CURVE_END_Q8_MAX) {
                end_q8 = ANALOG_CURVE_END_Q8_MAX;
            }
        }
        out_params->curve_end_q8 = end_q8;
    }

    out_params->nominal_bottom = k_analog_switch_bottom_adc[switch_type];
}

/* Read file "sw" into *sw_profiles. A file without version (or version 0) is converted: its travel
 * offsets are doubled. Missing switch types are filled in. Returns 1 on success, -1 when the file
 * system is not mounted, else the result of storage_read_pb_file. */
int analog_load_switch_profiles(SwitchProfiles *sw_profiles)
{
    char path[4];
    int result;

    if (g_storage_mounted == 0u) {
        return -1;
    }
    strcpy(path, "sw");
    result = storage_read_pb_file(path, &SwitchProfiles_msg, sw_profiles);
    if (result != 1) {
        return result;
    }

    if (!sw_profiles->has_version || sw_profiles->version == 0u) {
        /* Old file: travel offsets were stored with half the resolution. */
        if (sw_profiles->has_default_params && sw_profiles->default_params.has_travel_offset) {
            sw_profiles->default_params.travel_offset *= 2u;
        }
        for (uint8_t i = 0u; i < sw_profiles->entries_count; i++) {
            SwitchProfileEntry *entry = &sw_profiles->entries[i];
            SwitchKeyGroup *group = &entry->kind.key_group;

            if (entry->which_kind == 1u && group->has_params && group->params.has_travel_offset) {
                group->params.travel_offset *= 2u;
            }
        }
    }
    sw_profiles->has_version = true;
    sw_profiles->version = 1u;
    analog_fill_missing_switch_types(sw_profiles);
    return 1;
}

/* Apply g_analog_switch_profiles: build the keyboard-wide default parameters and the parameters
 * of every key group, then (outside the start-up sweep) derive bottom_level of every key from its
 * stored rest level and reset the bottom-out learning. */
void analog_apply_switch_profiles(void)
{
    analog_switch_params_t group_params;

    for (uint32_t pos = 0u; pos < ANALOG_KEY_COUNT; pos++) {
        g_analog_key_states[pos].has_switch_params = 0u;
    }

    analog_build_switch_params(&g_analog_default_switch_params, &g_analog_switch_profiles.default_params);

    for (uint8_t i = 0u; i < g_analog_switch_profiles.entries_count; i++) {
        const SwitchProfileEntry *entry = &g_analog_switch_profiles.entries[i];
        const SwitchKeyGroup *group = &entry->kind.key_group;
        uint32_t key_count;

        if (entry->which_kind != 1u) {
            continue;
        }
        analog_build_switch_params(&group_params, &group->params); /* has_params is not checked */
        key_count = group->key_ids_count;
        if (key_count == 0u) {
            continue;
        }
        for (uint8_t k = 0u; k < key_count; k++) {
            uint32_t key_id = group->key_ids[k];
            uint32_t col = key_id & 0x1Fu;
            /* KNOWN ISSUE: a key id outside the matrix or without sensor leaves the destination
             * NULL and the 16 bytes are still written, to address 0 (the pointer is volatile only
             * to keep the compiler from turning the store into a trap). Matches stock firmware
             * 2.14.1. */
            analog_switch_params_t *volatile dst = NULL;

            if ((key_id >> 5) < LAYOUT_ROWS && col < LAYOUT_COLS) {
                uint32_t scan_pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * (key_id >> 5) + col];

                if (scan_pos != ANALOG_NO_SCAN_POS) {
                    g_analog_key_states[scan_pos].has_switch_params = 1u;
                    dst = &g_analog_key_states[scan_pos].switch_params;
                }
            }
            *dst = group_params;
        }
    }

    if (g_adc_scan_calibrating != 0u) {
        return; /* the start-up sweep derives the levels itself */
    }

    for (uint32_t pos = 0u; pos < ANALOG_KEY_COUNT; pos++) {
        analog_key_state_t *key = &g_analog_key_states[pos];
        const analog_switch_params_t *params =
            key->has_switch_params != 0u ? &key->switch_params : &g_analog_default_switch_params;
        uint32_t rest = key->rest_level_single;
        /* round(curve_end_q8 * (nominal_bottom - rest) / 0xFFF00), unsigned arithmetic */
        uint32_t scaled = params->curve_end_q8 * (uint32_t)(params->nominal_bottom - rest)
                          + ANALOG_CURVE_END_Q8_MAX / 2u;
        uint32_t distance = ANALOG_MIN_BOTTOM_DISTANCE;

        if (scaled >= (ANALOG_MIN_BOTTOM_DISTANCE + 1u) * ANALOG_CURVE_END_Q8_MAX) {
            distance = scaled / ANALOG_CURVE_END_Q8_MAX;
        }
        key->bottom_level = (uint16_t)(rest + distance);
        key->bottom_is_learned = 0u;
        memset(&key->bo_tracker, 0, sizeof(key->bo_tracker));
        key->bo_reference = ANALOG_VALUE_NONE;
        key->bo_tracker.lower_bound = ANALOG_VALUE_NONE;
        key->bo_estimate = ANALOG_VALUE_NONE;
    }
}

/* Travel units (0..4095 scale) per 0.1 mm of the key at row / col, from its switch type and travel
 * offset: 2 * 4095 / remaining travel in 0.05 mm steps. Returns 112 for a position without key. */
uint32_t analog_get_key_travel_per_0p1mm(uint32_t row, uint32_t col)
{
    const analog_switch_params_t *params;
    uint32_t scan_pos;
    uint32_t travel;
    uint32_t offset;
    uint8_t remaining;

    if (row >= LAYOUT_ROWS || col >= LAYOUT_COLS) {
        return ANALOG_TRAVEL_PER_0P1MM_DEFAULT;
    }
    scan_pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];
    if (scan_pos == ANALOG_NO_SCAN_POS) {
        return ANALOG_TRAVEL_PER_0P1MM_DEFAULT;
    }
    if (g_analog_key_states[scan_pos].has_switch_params != 0u) {
        params = &g_analog_key_states[scan_pos].switch_params;
    } else {
        params = &g_analog_default_switch_params;
    }

    travel = k_analog_switch_travel[params->sw.type.switch_type];
    offset = params->sw.has_travel_offset ? params->sw.travel_offset : 0u;
    remaining = travel > offset ? (uint8_t)(travel - offset) : 0u;

    /* A remaining travel of 0 yields 0. */
    return remaining != 0u ? 0x1FFEu / remaining : 0u;
}

/* Convert the current sensor value of a key to travel: 0 while the key is at rest, else the
 * position between rest_level and bottom_level mapped through the travel curve of the switch type
 * to 0..4095. */
uint16_t analog_compute_key_travel(const analog_key_state_t *key)
{
    const analog_switch_params_t *params;
    const uint16_t *curve;
    uint32_t rest = key->rest_level;
    uint32_t value = key->value;
    uint32_t range;
    uint32_t curve_end_q8;
    uint32_t index_q8;
    uint32_t curve_end_value;
    int32_t travel;

    if (value <= (uint16_t)(rest + ANALOG_REST_MARGIN)) {
        return 0u;
    }
    params = key->has_switch_params != 0u ? &key->switch_params : &g_analog_default_switch_params;
    curve_end_q8 = params->curve_end_q8;
    range = (uint16_t)(key->bottom_level - rest);
    /* A zero range yields index 0. bottom_level == rest_level is reachable: the scan paths clamp
     * bottom_level to rest_level and copy bo_estimate into it unchecked. */
    index_q8 = range != 0u ? ((range >> 1) + curve_end_q8 * (value - rest)) / range : 0u;
    curve = k_analog_travel_curves[k_analog_switch_curve_index[params->sw.type.switch_type]];
    if (index_q8 >= curve_end_q8) {
        index_q8 = curve_end_q8;
    }

    travel = analog_lut_interpolate(curve, index_q8);
    if (params->sw.has_travel_offset) {
        /* Shortened travel: rescale so that the curve end reads as full travel. The divisor is 0
         * when the travel offset equals the total travel of the switch (curve_end_q8 == 0); the
         * result is then 0. */
        curve_end_value = analog_lut_interpolate(curve, curve_end_q8);
        travel = curve_end_value != 0u
                     ? (int32_t)((uint32_t)(ANALOG_CURVE_VALUE_MAX * travel) / curve_end_value)
                     : 0;
        if (travel >= (int32_t)ANALOG_CURVE_VALUE_MAX) {
            travel = (int32_t)ANALOG_CURVE_VALUE_MAX;
        }
    }
    return (uint16_t)((uint32_t)(ANALOG_TRAVEL_MAX * travel) / ANALOG_CURVE_VALUE_MAX);
}
