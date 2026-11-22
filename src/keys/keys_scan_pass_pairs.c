/*
 * keys_scan_pass_pairs.c - two-key advanced keys (step S8 of keys_process_scan_pass): after all
 * scan events of the pass are in, decide for every two-key binding which of its keys are
 * reported as pressed, and send their key actions.
 *
 *   kind 4  keys_resolve_depth_pair  the deeper key wins ("Rappy Snappy" in Wootility)
 *   kind 5  keys_resolve_socd_pair   SOCD: a fixed rule decides ("Snappy Tappy" in Wootility)
 *
 * The input stage (keys_pair_capture in keys_scan_pass_advanced.c) has stored the pressed state
 * and travel of both keys in the entry's run-time state.
 */
#include "keys/keys_scan_pass_internal.h"
#include "keys/keys.h"
#include "hid/hid.h"

/* Applies a resolution rule to two keys that are both pressed: clears the flag of the key that
 * loses. Unknown rules turn both keys off. */
static void keys_pair_apply_rule(uint32_t rule, uint32_t last_pressed, uint8_t *on1, uint8_t *on2)
{
    switch (rule) {
    case KEYS_PAIR_MODE_BOTH:
        break;
    case KEYS_PAIR_MODE_KEY1:
        *on2 = 0;
        break;
    case KEYS_PAIR_MODE_KEY2:
        *on1 = 0;
        break;
    case KEYS_PAIR_MODE_LAST_PRESS:
        if (last_pressed == 1) {
            *on2 = 0;
        } else if (last_pressed == 2) {
            *on1 = 0;
        } else {
            *on1 = 0;
            *on2 = 0;
        }
        break;
    default:  /* KEYS_PAIR_MODE_NEITHER and everything else */
        *on1 = 0;
        *on2 = 0;
        break;
    }
}

/* Reports one key of a pair: key-state entry {advanced key, kind}, then the keymap action of the
 * key on the entry's layer, pressed only if the resolver left the key on. A key flagged
 * `defer_output` (it interrupted a pending mod-tap in this pass) is reported released once; the
 * flag is consumed here. */
static void keys_pair_emit_key(keys_scan_pass_t *pass, uint32_t kind, uint8_t key_id, analog_key_state_t *key,
                               uint8_t layer, const uint16_t *travel, uint8_t on)
{
    uint16_t action;
    uint8_t pressed;

    hid_keystate_report_add_key(KEYS_PASS_KEYSTATE_ADVANCED(kind), *travel, key_id,
                                (key->flags & ANALOG_KEY_FLAG_PRESSED) != 0);
    action = keys_pass_layer_action(key, layer);
    pressed = on ? (uint8_t)((key->flags ^ ANALOG_KEY_FLAG_DEFER_OUTPUT) >> 7) : 0u;
    keys_apply_key_action(&pass->ctx, action, key_id, layer, *travel, 0, pressed);
    key->flags &= (uint8_t)~ANALOG_KEY_FLAG_DEFER_OUTPUT;
}

/* Reports both keys of a pair: a key with travel 0 is skipped, the second key also when its id
 * does not map to a key. */
static void keys_pair_emit(keys_scan_pass_t *pass, const AdvancedKey *entry, keys_advanced_key_state_t *state,
                           analog_key_state_t *key1, uint8_t on1, uint8_t on2)
{
    uint32_t kind = keys_pass_ak_kind(entry);
    uint8_t key2_id = entry->kind.depth_priority.second_key_id;

    /* KNOWN ISSUE: key1 is NULL when the entry's own key id does not map to a scan position,
     * and is used unchecked here. No key is bound as first key of such an entry, so its travel
     * normally stays 0 and the pointer is not used. Matches stock firmware 2.14.1. */
    if (state->pair.key[0].travel != 0)
        keys_pair_emit_key(pass, kind, entry->key_id, key1, entry->layer, &state->pair.key[0].travel, on1);

    if (state->pair.key[1].travel != 0) {
        analog_key_state_t *key2 = keys_pass_key_record(key2_id);

        if (key2 != NULL)
            keys_pair_emit_key(pass, kind, key2_id, key2, entry->layer, &state->pair.key[1].travel, on2);
    }
}

/* Kind 4: while both keys are pressed the deeper one wins. At about the same depth the previous
 * decision is kept, except when both are bottomed out: then the entry's rule decides. */
static void keys_resolve_depth_pair(keys_scan_pass_t *pass, const AdvancedKey *entry,
                                    keys_advanced_key_state_t *state, analog_key_state_t *key1)
{
    uint8_t on1 = state->pair.key[0].pressed;
    uint8_t on2 = state->pair.key[1].pressed;

    if (on1 != 0 && on2 != 0) {
        uint32_t travel1 = state->pair.key[0].travel;
        uint32_t travel2 = state->pair.key[1].travel;

        /* From here on the pressed byte of key 1 is carried as the decision of key 2 and ends
         * up in state byte +10 (matches stock firmware 2.14.1). Both bytes are 1 unless the
         * state was corrupted. */
        on2 = on1;
        if ((uint32_t)((int16_t)travel1 - (int32_t)travel2 + (int32_t)KEYS_PAIR_SAME_DEPTH) <= 2u * KEYS_PAIR_SAME_DEPTH) {
            if (travel1 > KEYS_PAIR_FULL_TRAVEL && travel2 > KEYS_PAIR_FULL_TRAVEL) {
                keys_pair_apply_rule(entry->kind.depth_priority.both_bottomed_resolution,
                                     state->pair.last_pressed, &on1, &on2);
            } else {
                on1 = state->pair.out1_or_latch;
                on2 = state->pair.out2;
            }
        } else if (travel1 <= travel2) {
            on1 = 0;
        } else {
            on2 = 0;
        }
    }

    keys_pair_emit(pass, entry, state, key1, on1, on2);

    state->pair.out1_or_latch = on1;
    state->pair.out2 = on2;
}

/* Kind 5: while both keys are pressed the entry's rule decides which one is reported. With the
 * option "both on when fully pressed" both are reported while both are bottomed out; the state
 * byte +9 latches that situation, and when one key leaves it the key that is still bottomed out
 * counts as the one pressed last. */
static void keys_resolve_socd_pair(keys_scan_pass_t *pass, const AdvancedKey *entry,
                                   keys_advanced_key_state_t *state, analog_key_state_t *key1)
{
    const TwoKeySocd *socd = &entry->kind.socd;
    uint8_t on1 = state->pair.key[0].pressed;
    uint8_t on2 = state->pair.key[1].pressed;

    if (on1 != 0 && on2 != 0) {
        uint32_t travel1 = state->pair.key[0].travel;
        uint32_t travel2 = state->pair.key[1].travel;
        bool both_on = socd->both_on_when_fully_pressed;

        if (both_on && travel1 > KEYS_PAIR_FULL_TRAVEL && travel2 > KEYS_PAIR_FULL_TRAVEL) {
            /* Both stay on. */
            state->pair.out1_or_latch = 1;
        } else {
            if (state->pair.out1_or_latch != 0) {
                if (travel1 <= KEYS_PAIR_FULL_TRAVEL) {
                    state->pair.out1_or_latch = 0;
                    if (travel2 > KEYS_PAIR_FULL_TRAVEL)
                        state->pair.last_pressed = 2;
                } else if (!both_on || travel2 <= KEYS_PAIR_FULL_TRAVEL) {
                    /* Without the option the latch can only be left over from a time the option
                     * was set; key 2 is not looked at then. */
                    state->pair.out1_or_latch = 0;
                    state->pair.last_pressed = 1;
                }
            }
            /* When a key just left the "both fully pressed" situation, "last pressed" was set
             * above and the rule is applied with it. */
            keys_pair_apply_rule(socd->resolution, state->pair.last_pressed, &on1, &on2);
        }
    }

    keys_pair_emit(pass, entry, state, key1, on1, on2);
}

/* S8: all two-key entries whose two key ids differ. The list length is read again in every
 * round. */
void keys_pass_resolve_two_key_pairs(keys_scan_pass_t *pass)
{
    for (uint8_t index = 0; index < g_keys_advanced_keys.entries_count; index++) {
        const AdvancedKey *entry = &g_keys_advanced_keys.entries[index];
        keys_advanced_key_state_t *state = &g_keys_advanced_key_states[index];
        analog_key_state_t *key1 = keys_pass_key_record(entry->key_id);
        uint8_t kind = keys_pass_ak_kind(entry);

        if (kind != KEYS_AK_PAIR_DEEP && kind != KEYS_AK_PAIR_SOCD)
            continue;
        if (entry->kind.depth_priority.second_key_id == entry->key_id)
            continue;

        if (kind == KEYS_AK_PAIR_DEEP)
            keys_resolve_depth_pair(pass, entry, state, key1);
        else
            keys_resolve_socd_pair(pass, entry, state, key1);
    }
}
