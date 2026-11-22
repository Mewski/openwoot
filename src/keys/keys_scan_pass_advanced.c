/*
 * keys_scan_pass_advanced.c - input stages of the advanced keys (step S4 of
 * keys_process_scan_pass) and the resolution of pending mod-tap keys by another key press
 * (part of S5).
 *
 * An advanced key is one entry of g_keys_advanced_keys (profile file "ak2") with 12 bytes of
 * run-time state in g_keys_advanced_key_states. The input stage runs once per scan event of a key
 * that has an entry bound on its current layer and turns the travel into state changes; the
 * output stage (keys_process_advanced_keys in keys_advanced.c) turns the state into key
 * actions. The input stage reports whether the entry is "active", which is stored in the key
 * record so that the entry keeps being processed while the key is at rest or on another layer.
 *
 *   kind 1  keys_dks_input          dynamic keystroke: four bindings, four travel events
 *   kind 2  keys_modtap_input       tap key / hold key
 *   kind 3  keys_toggle_input       a tap latches the key on, the next tap releases it
 *   kind 4, 5  keys_pair_capture    two-key bindings: record both keys for the resolvers of S8
 */
#include <string.h>

#include "keys/keys_scan_pass_internal.h"
#include "keys/keys.h"
#include "analog/analog.h"
#include "rgb/rgb_types.h"
#include "rgb/rgb_data.h"
#include "rgb/rgb_frame.h"
#include "tables/analog_tables.h"

/* Travel distances of the dynamic keystroke stage, in travel units 0..4095. */
#define KEYS_DKS_BOTTOM_MIN_GAP      112u  /* the bottom-out point is at least this far behind the actuation point */
#define KEYS_DKS_RELEASE_HYSTERESIS  80u   /* release this far before the actuation point, lift this far before the bottom-out point */
#define KEYS_DKS_RELEASE_FLOOR       1u    /* release threshold when the actuation point is 81 or less */

/* Overlay colour of a latched toggle key that is not a layer key: all channels at maximum. */
#define KEYS_TOGGLE_INDICATOR_COLOR  0xFFFFFFFFu

/* ---- S4a: dynamic keystroke --------------------------------------------------------------------- */

/* Last travel value before the end of the switch's travel: one unit of k_analog_switch_travel
 * (0.05 mm) short of the full travel. */
static uint32_t keys_dks_travel_limit(const analog_key_state_t *key)
{
    const analog_switch_params_t *sw = key->has_switch_params ? &key->switch_params
                                                               : &g_analog_default_switch_params;
    uint32_t total = k_analog_switch_travel[sw->sw.type.switch_type];
    uint32_t offset = sw->sw.has_travel_offset ? sw->sw.travel_offset : 0u;
    uint8_t range = (total > offset) ? (uint8_t)(total - offset) : 0u;
    uint32_t last_step = (uint8_t)(range - 1u);

    if (last_step >= range)
        last_step = range;
    /* KNOWN ISSUE: a travel offset as large as the switch's travel gives a range of 0 and a
     * division by zero. The Cortex-M4 UDIV instruction returns 0 for that (DIV_0_TRP is
     * clear), which is written out here because the C division would be undefined. Matches
     * stock firmware 2.14.1. */
    if (range == 0)
        return 0;
    return (uint16_t)(ANALOG_TRAVEL_MAX * last_step / range);
}

/* Dynamic keystroke: the key passes four points of its travel (actuation, bottom-out, leaving the
 * bottom, release) and each point queues press / release / tap events for up to four bindings.
 * This stage does not use keys_update_press_state: it drives the pressed / changed flags of the
 * key record itself. Returns 1 while the entry is active. */
static uint8_t keys_dks_input(keys_scan_pass_t *pass, keys_key_ref_t *ref, uint32_t travel,
                              const AdvancedKey *entry, keys_advanced_key_state_t *state)
{
    const DynamicKeystroke *dks = &entry->kind.dynamic_keystroke;
    analog_key_state_t *key = ref->key_state;
    uint32_t point = dks->bottom_out_point;
    uint32_t limit;
    int32_t bottom;
    uint32_t actuation;
    uint16_t bottom_threshold;
    uint32_t release_threshold;

    /* Bottom-out point in travel units. */
    if (!dks->has_bottom_out_point) {
        pass->dks_bottom_travel = KEYS_DKS_DEFAULT_BOTTOM;
    } else if (point <= KEYS_DISTANCE_LEGACY_MAX) {
        pass->dks_bottom_travel = (uint16_t)(ANALOG_TRAVEL_MAX * point / KEYS_DISTANCE_LEGACY_MAX);
    } else if (KEYS_DISTANCE_IS_Q14(point)) {
        pass->dks_bottom_travel = (uint16_t)(ANALOG_TRAVEL_MAX * (point & KEYS_DISTANCE_Q14_MASK) / KEYS_DISTANCE_Q14_MASK);
    }
    /* KNOWN ISSUE: a stored point that is neither encoding leaves the value untouched, i.e. the
     * bottom-out point of the previous dynamic keystroke event of this pass, or 0 if there was
     * none (the value is zeroed once per pass). Matches stock firmware 2.14.1. */

    limit = keys_dks_travel_limit(key);
    bottom = (int32_t)pass->dks_bottom_travel;
    if (pass->dks_bottom_travel >= limit)
        bottom = (int32_t)limit;

    actuation = keys_get_actuation_threshold(ref);
    bottom_threshold = (uint16_t)(actuation + KEYS_DKS_BOTTOM_MIN_GAP);
    if ((int32_t)(actuation + KEYS_DKS_BOTTOM_MIN_GAP) < bottom)
        bottom_threshold = (uint16_t)bottom;
    if ((int16_t)actuation <= (int16_t)(KEYS_DKS_RELEASE_HYSTERESIS + 1u))
        release_threshold = KEYS_DKS_RELEASE_FLOOR;
    else
        release_threshold = (uint16_t)(actuation - KEYS_DKS_RELEASE_HYSTERESIS);

    switch (state->dks.phase) {
    case KEYS_DKS_IDLE:
        if (travel >= actuation) {
            state->dks.phase = KEYS_DKS_PRESSED;
            keys_dks_queue_events(state, dks->event_masks[0]);
            key->flags = (uint8_t)((key->flags & ~(ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED)) |
                                   ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED);
        }
        break;
    case KEYS_DKS_PRESSED:
    case KEYS_DKS_LIFTED:
        if (travel >= bottom_threshold) {
            state->dks.phase = KEYS_DKS_BOTTOMED;
            keys_dks_queue_events(state, dks->event_masks[1]);
        } else if (travel < release_threshold) {
            state->dks.phase = KEYS_DKS_IDLE;
            keys_dks_queue_events(state, dks->event_masks[3]);
            key->flags = (uint8_t)((key->flags & ~(ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED)) |
                                   ANALOG_KEY_FLAG_CHANGED);
        }
        break;
    case KEYS_DKS_BOTTOMED:
        if ((int32_t)travel < (int16_t)bottom_threshold - (int32_t)KEYS_DKS_RELEASE_HYSTERESIS) {
            state->dks.phase = KEYS_DKS_LIFTED;
            keys_dks_queue_events(state, dks->event_masks[2]);
        }
        break;
    default:
        break;
    }

    /* Active while a binding still has queued events or the key is not back at rest. */
    if ((state->dks.queue[0].events | state->dks.queue[1].events |
         state->dks.queue[2].events | state->dks.queue[3].events) != 0)
        return 1;
    return state->dks.phase != KEYS_DKS_IDLE;
}

/* ---- S4b: mod-tap ------------------------------------------------------------------------------- */

/* A pressed mod-tap key is put on the pending list when another key press shall decide it:
 * always for the interrupt modes "hold" and "tap", never for "ignore", otherwise only when the
 * hold key code is a modifier or a momentary layer key. */
static bool keys_modtap_wants_slot(const ModTap *modtap)
{
    uint32_t hold = modtap->hold_keycode;

    if (modtap->has_interrupt_mode) {
        uint32_t mode = modtap->interrupt_mode;

        if (mode == KEYS_MODTAP_INTERRUPT_NEVER)
            return false;
        if (mode - KEYS_MODTAP_INTERRUPT_HOLD <= KEYS_MODTAP_INTERRUPT_TAP - KEYS_MODTAP_INTERRUPT_HOLD)
            return true;
    }
    return hold - LAYOUT_KEYCODE_MOD_FIRST <= LAYOUT_KEYCODE_MOD_LAST - LAYOUT_KEYCODE_MOD_FIRST ||
           hold == LAYOUT_KEYCODE_LAYER1 ||
           (uint8_t)(hold - LAYOUT_KEYCODE_LAYER2) <= LAYOUT_KEYCODE_LAYER3 - LAYOUT_KEYCODE_LAYER2;
}

/* Takes the first free pending slot for the key; returns its index, or -1 when all five are in
 * use. */
static int8_t keys_modtap_alloc_slot(const keys_key_ref_t *ref)
{
    for (int8_t index = 0; index < (int8_t)KEYS_MODTAP_SLOT_COUNT; index++) {
        keys_modtap_slot_t *slot = &g_keys_modtap_slots[index];

        if (slot->flags & KEYS_MODTAP_SLOT_IN_USE)
            continue;
        slot->flags = (uint8_t)((slot->flags & 0xF0u) | KEYS_MODTAP_SLOT_IN_USE | KEYS_MODTAP_SLOT_FREE_ON_RESOLVE);
        slot->key = *ref;
        if (index > g_keys_modtap_slot_top)
            g_keys_modtap_slot_top = index;
        return index;
    }
    return -1;
}

/* Mod-tap: the key sends its tap key code when released before the hold time and its hold key
 * code when held longer. This stage handles the edges; the timer runs in the output stage
 * (keys_process_advanced_keys case 2), which also turns "undecided" into "hold" and ends a tap. */
static uint8_t keys_modtap_input(keys_key_ref_t *ref, uint32_t travel,
                                 const AdvancedKey *entry, keys_advanced_key_state_t *state)
{
    const ModTap *modtap = &entry->kind.mod_tap;
    uint8_t flags;
    bool pressed;

    keys_update_press_state(ref, travel);
    flags = ref->key_state->flags;
    pressed = (flags & ANALOG_KEY_FLAG_PRESSED) != 0;

    if (flags & ANALOG_KEY_FLAG_CHANGED) {
        if (!pressed) {
            /* Release edge: the key no longer waits for another key press. */
            if (state->modtap.slot != -1) {
                keys_modtap_release_slot(state->modtap.slot);
                state->modtap.slot = -1;
            }
        } else if (keys_modtap_wants_slot(modtap)) {
            /* Press edge. */
            state->modtap.slot = keys_modtap_alloc_slot(ref);
        }

        switch (state->modtap.state) {
        case KEYS_MODTAP_IDLE:
            if (pressed) {
                state->modtap.state = KEYS_MODTAP_UNDECIDED;
                state->modtap.timer_ms = KEYS_MODTAP_TIMER_IDLE;
            }
            break;
        case KEYS_MODTAP_UNDECIDED: {
            uint32_t hold_ms = modtap->has_hold_time_10ms ? KEYS_MODTAP_HOLD_UNIT_MS * modtap->hold_time_10ms
                                                          : KEYS_MODTAP_DEFAULT_HOLD_MS;

            if (state->modtap.timer_ms >= hold_ms) {
                state->modtap.state = KEYS_MODTAP_HOLD;
                state->modtap.timer_ms = 0;
            } else if (!pressed) {
                /* Released early: tap. */
                state->modtap.state = KEYS_MODTAP_TAP;
                state->modtap.timer_ms = KEYS_MODTAP_TIMER_IDLE;
            }
            break;
        }
        case KEYS_MODTAP_HOLD:
            if (!pressed) {
                state->modtap.state = KEYS_MODTAP_IDLE;
                state->modtap.timer_ms = 0;
            }
            break;
        default:
            break;
        }
    }

    return state->modtap.state != KEYS_MODTAP_IDLE;
}

/* ---- S5: pending mod-tap keys resolved by another key press -------------------------------------- */

/* One pending slot against the key that was just pressed. Returns true when the mod-tap key was
 * decided and its key action sent. */
static bool keys_modtap_resolve_slot(keys_scan_pass_t *pass, const keys_key_ref_t *ref, int8_t slot_index)
{
    keys_modtap_slot_t *slot = &g_keys_modtap_slots[slot_index];
    analog_key_state_t *modtap_key;
    const AdvancedKey *entry;
    keys_advanced_key_state_t *state;
    uint32_t ak_index;
    uint32_t presser_index;
    uint8_t slot_flags;
    bool resolved = false;

    /* The pending key itself does not interrupt. */
    if (slot->key.key_id == ref->key_id)
        return false;

    /* The pending key must still be a mod-tap on its current layer. */
    ak_index = slot->key.key_state->ak_slot[keys_get_binding_layer(&slot->key)].index;
    if (g_keys_advanced_keys.entries_count <= ak_index || ak_index == ANALOG_NO_ADVANCED_KEY)
        return false;
    entry = &g_keys_advanced_keys.entries[ak_index];
    if (keys_pass_ak_kind(entry) != KEYS_AK_MODTAP)
        return false;
    modtap_key = slot->key.key_state;

    /* Another mod-tap key does not interrupt. (The index is not checked against the list
     * length here.) */
    presser_index = ref->key_state->ak_slot[keys_get_binding_layer(ref)].index;
    if (presser_index != ANALOG_NO_ADVANCED_KEY &&
        g_keys_advanced_keys.entries[presser_index].which_kind == KEYS_AK_MODTAP)
        return false;

    state = &g_keys_advanced_key_states[ak_index];
    if (state->modtap.state == KEYS_MODTAP_UNDECIDED) {
        uint8_t entry_layer = entry->layer;

        if (((modtap_key->flags2 & ANALOG_KEY_FLAG2_LATCHED_LAYER_MASK) >> ANALOG_KEY_FLAG2_LATCHED_LAYER_SHIFT) == entry_layer) {
            /* Decide now: hold, or tap in the interrupt mode "tap", and send the
             * key action at once so that the host sees it before the interrupting key. */
            const ModTap *modtap = &entry->kind.mod_tap;
            uint32_t keycode = modtap->hold_keycode;
            uint8_t new_state = KEYS_MODTAP_HOLD;
            uint16_t travel;

            if (modtap->has_interrupt_mode && (uint32_t)modtap->interrupt_mode == KEYS_MODTAP_INTERRUPT_TAP) {
                keycode = modtap->tap_keycode;
                new_state = KEYS_MODTAP_TAP;
            }
            state->modtap.state = new_state;
            state->modtap.timer_ms = 0;
            resolved = true;

            travel = analog_compute_key_travel(modtap_key);
            keys_apply_key_action(&pass->ctx, keys_pass_keycode_action(keycode), slot->key.key_id,
                                  entry_layer, travel, 1, 1);
        }
    }

    slot_flags = slot->flags;
    state->modtap.slot = -1;
    if (slot_flags & KEYS_MODTAP_SLOT_FREE_ON_RESOLVE)
        keys_modtap_release_slot(slot_index);
    return resolved;
}

/* A key was pressed (ref): walk the pending mod-tap keys and decide those the press interrupts.
 * If one was decided, the pressing key is flagged `defer_output` and gives no output in this
 * pass. */
void keys_pass_modtap_resolve_pending(keys_scan_pass_t *pass, keys_key_ref_t *ref)
{
    uint8_t deferred = 0;

    if (g_keys_modtap_slot_top < 0)
        return;

    /* The top index is read again in every round: releasing a slot lowers it. */
    for (int8_t slot_index = 0; slot_index <= g_keys_modtap_slot_top; slot_index++) {
        uint8_t slot_flags = g_keys_modtap_slots[slot_index].flags;

        /* Bits 3:2 are never set by this firmware. */
        if ((slot_flags & KEYS_MODTAP_SLOT_IN_USE) && !(slot_flags & KEYS_MODTAP_SLOT_UNK_MASK)) {
            if (keys_modtap_resolve_slot(pass, ref, slot_index))
                deferred = 1;
        }
    }

    if (deferred)
        ref->key_state->flags |= ANALOG_KEY_FLAG_DEFER_OUTPUT;
}

/* ---- S4c: toggle -------------------------------------------------------------------------------- */

/* Toggle: a press shorter than 200 ms flips a latch; while the latch is on the output stage keeps
 * the key code pressed and the key shows an overlay colour. A longer press acts as a plain key
 * and clears the latch. The press time is counted by the output stage. */
static uint8_t keys_toggle_input(keys_key_ref_t *ref, uint32_t travel,
                                 const AdvancedKey *entry, keys_advanced_key_state_t *state)
{
    uint32_t keycode = entry->kind.toggle.keycode;
    uint8_t flags;

    keys_update_press_state(ref, travel);
    flags = ref->key_state->flags;
    state->toggle.pressed = (flags & ANALOG_KEY_FLAG_PRESSED) != 0;

    if (!(flags & ANALOG_KEY_FLAG_CHANGED))
        return state->toggle.latched;
    if (flags & ANALOG_KEY_FLAG_PRESSED) {
        state->toggle.hold_ms = 0;
        return state->toggle.latched;
    }

    /* Release edge. */
    if (state->toggle.hold_ms <= KEYS_TOGGLE_TAP_MAX_MS) {
        state->toggle.latched ^= 1u;
        if (state->toggle.latched != 0) {
            rgb_overlay_params_t params;
            uint32_t color;

            /* Layer keys use the layer-lock indicator colour of the profile. */
            if (keycode == LAYOUT_KEYCODE_LAYER1 ||
                (uint8_t)(keycode - LAYOUT_KEYCODE_LAYER2) <= LAYOUT_KEYCODE_LAYER3 - LAYOUT_KEYCODE_LAYER2)
                color = rgb_unpack_color(g_rgb_settings.base.layer_lock_color);
            else
                color = KEYS_TOGGLE_INDICATOR_COLOR;

            /* Overlay type 0 (one key in one colour); the overlay id is the key id. */
            memset(&params, 0, sizeof(params));
            params.u.solid_key.key_pos = ref->key_id;
            memcpy(&params.u.solid_key.color, &color, sizeof(color));

            if (ref->key_id == 0)
                return state->toggle.latched;
            /* The SDK flag itself is returned here (matches stock firmware 2.14.1); both it and
             * the latch are non-zero. */
            if (g_rgb_sdk_mode_active != 0)
                return g_rgb_sdk_mode_active;
            rgb_overlay_start(ref->key_id, 0xFFFF, &params);
            return state->toggle.latched;
        }
    } else {
        state->toggle.latched = 0;
    }
    rgb_overlay_stop(ref->key_id);
    return state->toggle.latched;
}

/* ---- S4d: two-key bindings ---------------------------------------------------------------------- */

/* Two-key bindings (kinds 4 and 5): both keys carry the same entry. This stage only records the
 * pressed state and travel of the key the event belongs to and which key was pressed last; the
 * resolvers of S8 decide the output after all events of the pass are in. */
static uint8_t keys_pair_capture(keys_key_ref_t *ref, uint32_t travel,
                                 const AdvancedKey *entry, keys_advanced_key_state_t *state)
{
    /* Both kinds start with the id of the second key. */
    uint32_t second_key_id = entry->kind.depth_priority.second_key_id;
    uint32_t which = (ref->key_id == second_key_id) ? 1u : 0u;
    bool pressed;

    keys_update_press_state(ref, travel);
    pressed = (ref->key_state->flags & ANALOG_KEY_FLAG_PRESSED) != 0;

    if (state->pair.key[which].pressed == 0 && pressed)
        state->pair.last_pressed = (uint8_t)(which + 1u);
    state->pair.key[which].pressed = pressed;
    state->pair.key[which].travel = (uint16_t)travel;

    return state->pair.key[0].travel != 0 || state->pair.key[1].travel != 0;
}

/* ---- S4 ----------------------------------------------------------------------------------------- */

/* Input stage of the advanced key `ak_index` (already checked against the list length) for one
 * scan event; stores the "active" flag of the entry in the key record. */
void keys_pass_advanced_key_input(keys_scan_pass_t *pass, keys_key_ref_t *ref, uint32_t travel, uint32_t ak_index)
{
    const AdvancedKey *entry = &g_keys_advanced_keys.entries[ak_index];
    keys_advanced_key_state_t *state = &g_keys_advanced_key_states[ak_index];
    uint8_t entry_layer = entry->layer;
    uint8_t active;

    switch (keys_pass_ak_kind(entry)) {
    case KEYS_AK_DKS:
        active = keys_dks_input(pass, ref, travel, entry, state);
        break;
    case KEYS_AK_MODTAP:
        active = keys_modtap_input(ref, travel, entry, state);
        break;
    case KEYS_AK_TOGGLE:
        active = keys_toggle_input(ref, travel, entry, state);
        break;
    case KEYS_AK_PAIR_DEEP:
    case KEYS_AK_PAIR_SOCD:
        active = keys_pair_capture(ref, travel, entry, state);
        break;
    default:
        active = 0;
        break;
    }

    /* The layer of the entry is not range-checked here. */
    ref->key_state->ak_slot[entry_layer].active = active;
}
