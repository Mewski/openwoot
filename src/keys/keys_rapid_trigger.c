/*
 * keys_rapid_trigger.c - per-key rapid-trigger configuration.
 *
 * Profile file "prt2" (g_keys_rt_groups: groups of keys with own rapid-trigger parameters or an
 * on / off switch) and the legacy file "prt" (one mode byte per key) are turned into the
 * per-key override bits and parameter pointers of the key records. The press decision that
 * uses them is keys_update_press_state (keys_actuation.c).
 */
#include "keys/keys.h"
#include "analog/analog.h"
#include "analog/analog_data.h"
#include "layout/layout.h"
#include "tables/analog_tables.h"

/* Travel of 0.1 / 0.2 / 0.4 / 0.55 mm in 12-bit travel units is `numerator / switch travel`,
 * the switch travel being in 0.05 mm units. */
#define KEYS_RT_Q12_PER_0P1MM    0x1FFEu  /* minimum sensitivity */
#define KEYS_RT_Q12_PER_0P2MM    0x3FFCu  /* default press sensitivity when a release sensitivity exists */
#define KEYS_RT_Q12_PER_0P4MM    0x7FF8u  /* default sensitivity without separate release sensitivity */
#define KEYS_RT_Q12_PER_0P55MM   0xAFF5u  /* default release sensitivity */

/* `numerator / divisor`, with 0 for a zero divisor. That is what the UDIV instruction of the
 * Cortex-M4 returns while SCB->CCR.DIV_0_TRP is clear (this firmware never sets it), and what
 * stock firmware 2.14.1 computes. Spelled out because a C division by zero is undefined and GCC
 * replaces such a path by a trap instruction. */
static uint32_t keys_rt_udiv(uint32_t numerator, uint32_t divisor)
{
    return (divisor != 0u) ? numerator / divisor : 0u;
}

/* Key id -> scan position, ANALOG_NO_SCAN_POS when the id is out of range or has no key. */
static uint8_t keys_rt_key_id_to_scan_pos(uint8_t key_id)
{
    uint8_t col = KEYS_KEY_ID_COL(key_id);

    if (key_id > KEYS_KEY_ID_MAX || col > LAYOUT_COLS - 1u) {
        return ANALOG_NO_SCAN_POS;
    }
    return g_layout_keyid_to_scanpos[LAYOUT_COLS * KEYS_KEY_ID_ROW(key_id) + col];
}

/* Removes the per-key rapid-trigger overrides of all keys. */
static void keys_clear_rapid_trigger_overrides(void)
{
    for (uint32_t pos = 0; pos != ANALOG_KEY_COUNT; pos++) {
        g_analog_key_states[pos].flags &= (uint8_t)~(ANALOG_KEY_FLAG_RT_OVERRIDE | ANALOG_KEY_FLAG_RT_ON);
        g_analog_key_states[pos].rt_params = NULL;
    }
}

/* Flag bits of a key with a per-key override: override present + rapid trigger on / off. */
static uint8_t keys_rt_override_bits(bool enabled)
{
    return (uint8_t)(ANALOG_KEY_FLAG_RT_OVERRIDE | (enabled ? ANALOG_KEY_FLAG_RT_ON : 0u));
}

/* Sanitises a "prt2" message before it is stored or applied: in every group with own
 * rapid-trigger params the sensitivity (and the release sensitivity, if present) is brought
 * into the 14-bit distance encoding, replaced by a default when unusable and raised to the
 * minimum of 0.1 mm. Minimum and defaults (0.4 mm, or 0.2 mm press / 0.55 mm release) are
 * computed from the switch travel of the keys of the group. */
void keys_sanitize_rapid_trigger_groups(RapidTriggerGroups *rt_groups)
{
    /* The group and key indices are 8 bit wide. */
    for (uint8_t g = 0; g < rt_groups->groups_count; g++) {
        RapidTriggerGroup *group = &rt_groups->groups[g];
        uint32_t min_sensitivity = 0;
        uint32_t min_release = 0;
        uint32_t default_sensitivity = 0;
        uint32_t default_release = 0;

        if (group->which_setting != RapidTriggerGroup_params_tag || group->key_ids_count == 0) {
            continue;
        }

        /* Largest values over the keys of the group, i.e. those of the shortest switch. */
        for (uint8_t k = 0; k < group->key_ids_count; k++) {
            uint8_t pos = keys_rt_key_id_to_scan_pos(group->key_ids[k]);
            const analog_switch_params_t *switch_params;
            uint32_t total_travel;
            uint32_t travel_offset;
            uint32_t travel;
            uint32_t per_0p1mm;

            if (pos > ANALOG_KEY_COUNT - 1u) {
                continue;
            }
            if (g_analog_key_states[pos].has_switch_params) {
                switch_params = &g_analog_key_states[pos].switch_params;
            } else {
                switch_params = &g_analog_default_switch_params;
            }
            total_travel = k_analog_switch_travel[switch_params->sw.type.switch_type];
            travel_offset = switch_params->sw.has_travel_offset ? switch_params->sw.travel_offset : 0u;
            travel = (total_travel <= travel_offset) ? 0u : (uint8_t)(total_travel - travel_offset);

            /* KNOWN ISSUE: a travel offset that is not smaller than the switch travel gives
             * travel 0 and the divisions below divide by zero; the result is then 0, so such a
             * key contributes no minimum and no default. Matches stock firmware 2.14.1. */
            per_0p1mm = keys_rt_udiv(KEYS_RT_Q12_PER_0P1MM, travel);
            if (min_sensitivity < per_0p1mm) {
                min_sensitivity = per_0p1mm;
            }
            if (group->setting.params.has_release_sensitivity) {
                if (min_release < per_0p1mm) {
                    min_release = per_0p1mm;
                }
                if (default_sensitivity < keys_rt_udiv(KEYS_RT_Q12_PER_0P2MM, travel)) {
                    default_sensitivity = keys_rt_udiv(KEYS_RT_Q12_PER_0P2MM, travel);
                }
                if (default_release < keys_rt_udiv(KEYS_RT_Q12_PER_0P55MM, travel)) {
                    default_release = keys_rt_udiv(KEYS_RT_Q12_PER_0P55MM, travel);
                }
            } else {
                if (default_sensitivity < keys_rt_udiv(KEYS_RT_Q12_PER_0P4MM, travel)) {
                    default_sensitivity = keys_rt_udiv(KEYS_RT_Q12_PER_0P4MM, travel);
                }
            }
        }

        keys_sanitize_distance(&group->setting.params.sensitivity, min_sensitivity, default_sensitivity);
        if (group->setting.params.has_release_sensitivity) {
            keys_sanitize_distance(&group->setting.params.release_sensitivity, min_release, default_release);
        }
    }
}

/* Applies g_keys_rt_groups (file "prt2") to the key records: removes all per-key overrides,
 * then gives every key of every group its override bits (rapid trigger on / off) and, for a
 * group with own params, the pointer to those params. A key listed in several groups takes
 * the setting of the last one. */
void keys_apply_rapid_trigger_groups(void)
{
    uint32_t group_count;

    keys_clear_rapid_trigger_overrides();

    group_count = g_keys_rt_groups.groups_count;
    for (uint8_t g = 0; g < group_count; g++) {
        RapidTriggerGroup *group = &g_keys_rt_groups.groups[g];
        RapidTriggerParams *params;
        bool enabled;
        uint32_t key_count;

        if (group->which_setting == RapidTriggerGroup_enabled_tag) {
            /* Only bit 0 of the stored byte is used; nanopb stores 0 or 1. */
            enabled = (*(const uint8_t *)&group->setting.enabled & 1u) != 0;
            params = NULL;
        } else {
            /* Every other oneof state, also "nothing set", is taken as a params record. */
            enabled = true;
            params = &group->setting.params;
        }

        key_count = group->key_ids_count;
        for (uint8_t k = 0; k < key_count; k++) {
            uint8_t pos = keys_rt_key_id_to_scan_pos(group->key_ids[k]);

            if (pos == ANALOG_NO_SCAN_POS) {
                continue;
            }
            g_analog_key_states[pos].flags =
                (g_analog_key_states[pos].flags & (uint8_t)~(ANALOG_KEY_FLAG_RT_OVERRIDE | ANALOG_KEY_FLAG_RT_ON))
                | keys_rt_override_bits(enabled);
            if (params != NULL) {
                g_analog_key_states[pos].rt_params = params;
            }
        }
    }
}

/* Applies a legacy "prt" message (one mode byte per key, as matrix or as sparse list of key id
 * | mode << 8): removes all per-key overrides, then sets the override bits of every key with a
 * non-zero mode. No key gets own params, so the profile-wide ones apply.
 * g_keys_rt_legacy_override_count is left as the list count plus the number of list items with
 * a non-zero mode. */
void keys_apply_legacy_rapid_trigger_keys(const KeyByteMap *modes)
{
    uint32_t item_count;
    uint8_t override_count;
    bool any_override = false;

    g_keys_rt_legacy_override_count = 0;
    keys_clear_rapid_trigger_overrides();

    if (modes->which_map == KeyByteMap_matrix_tag) {
        for (uint32_t row = 0; row != LAYOUT_ROWS; row++) {
            for (uint32_t col = 0; col != LAYOUT_KEYMAP_COLS; col++) {
                uint8_t mode = modes->map.matrix.rows[row].values[col];
                uint8_t pos;

                if (mode == KEYS_RT_LEGACY_NO_OVERRIDE || col > LAYOUT_COLS - 1u) {
                    continue;
                }
                pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];
                if (pos == ANALOG_NO_SCAN_POS) {
                    continue;
                }
                g_analog_key_states[pos].flags =
                    (g_analog_key_states[pos].flags & (uint8_t)~(ANALOG_KEY_FLAG_RT_OVERRIDE | ANALOG_KEY_FLAG_RT_ON))
                    | keys_rt_override_bits(mode == KEYS_RT_LEGACY_ON);
            }
        }
        /* KNOWN ISSUE: the matrix branch runs into the list code below, so the first bytes of
         * the matrix (rows 0 and following) are read once more as {uint16 count, {key id,
         * mode} items}. A non-zero mode at row 0, column 1 is the high byte of that count:
         * the 8-bit index below never reaches it and the loop does not end. Matches stock
         * firmware 2.14.1. */
    } else if (modes->which_map != KeyByteMap_list_tag) {
        return;
    }

    item_count = modes->map.list.items_count;
    override_count = (uint8_t)item_count;
    g_keys_rt_legacy_override_count = override_count;

    /* The index is 8 bit wide. */
    for (uint8_t i = 0; i < item_count; i++) {
        uint16_t item = modes->map.list.items[i];
        uint8_t key_id = (uint8_t)item;
        uint8_t mode = (uint8_t)(item >> 8);
        uint8_t pos;

        if (mode == KEYS_RT_LEGACY_NO_OVERRIDE) {
            continue;
        }
        override_count++;
        any_override = true;
        pos = keys_rt_key_id_to_scan_pos(key_id);
        if (pos == ANALOG_NO_SCAN_POS) {
            continue;
        }
        g_analog_key_states[pos].flags =
            (g_analog_key_states[pos].flags & (uint8_t)~(ANALOG_KEY_FLAG_RT_OVERRIDE | ANALOG_KEY_FLAG_RT_ON))
            | keys_rt_override_bits(mode == KEYS_RT_LEGACY_ON);
    }

    if (any_override) {
        g_keys_rt_legacy_override_count = override_count;
    }
}
