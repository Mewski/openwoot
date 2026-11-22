/*
 * keys_advanced.c - advanced keys of profile file "ak2" (g_keys_advanced_keys).
 *
 * Binding of the entries to the key records, validation, and the output stage of the run-time
 * state machines: dynamic keystroke, mod-tap, toggle and the two-key bindings. The input stage
 * (travel events, mod-tap resolution, two-key resolution) is in keys_process_scan_pass.
 */
#include "keys/keys.h"
#include "analog/analog.h"
#include "analog/analog_data.h"
#include "hid/hid.h"
#include "layout/layout.h"
#include "rgb/rgb_frame.h"
#include "tables/layout_keycode_tables.h"

/* Action kind used for the key-state report entry of an advanced key: logged as
 * HID_KEY_STATE_KIND_ADVANCED with the entry kind as code. */
#define KEYS_AK_REPORT_ACTION_KIND   5u
#define KEYS_MODTAP_CAPS_LOCK_TAP_MS 100u    /* tap length of a Caps Lock tap key code */
#define KEYS_TRAVEL_MAX              4095u

/* Frees a pending mod-tap slot. When it was the highest used slot, g_keys_modtap_slot_top moves
 * down to the next slot in use (-1 = none). Indices outside 0..top are ignored. */
void keys_modtap_release_slot(int slot)
{
    if (slot < 0 || g_keys_modtap_slot_top < slot) {
        return;
    }

    bool was_top = g_keys_modtap_slot_top == slot;

    g_keys_modtap_slots[slot].flags &= (uint8_t)~KEYS_MODTAP_SLOT_IN_USE;
    if (was_top) {
        do {
            slot = (int8_t)(slot - 1);
        } while (slot != -1 && (g_keys_modtap_slots[slot].flags & KEYS_MODTAP_SLOT_IN_USE) == 0);
        g_keys_modtap_slot_top = (int8_t)slot;
    }
}

/* Layer whose binding applies to a key: the layer of a latched toggle entry bound on the key
 * (looked for only while an advanced key of the key is active), else the layer latched when
 * the key was pressed, else, for a released key, the active layer. */
uint32_t keys_get_binding_layer(const keys_key_ref_t *key_ref)
{
    const analog_key_state_t *key = key_ref->key_state;
    bool any_active = false;

    for (uint32_t layer = 0; layer != ANALOG_KEY_LAYERS; layer++) {
        if (key->ak_slot[layer].index != ANALOG_NO_ADVANCED_KEY && key->ak_slot[layer].active != 0) {
            any_active = true;
            break;
        }
    }

    if (any_active) {
        /* A latched toggle key pins the key to the layer of its entry. The index is not
         * checked against the entry count. */
        for (uint32_t layer = 0; layer != ANALOG_KEY_LAYERS; layer++) {
            uint8_t index = key->ak_slot[layer].index;
            const AdvancedKey *entry = &g_keys_advanced_keys.entries[index];

            if (index != ANALOG_NO_ADVANCED_KEY && (uint8_t)entry->which_kind == KEYS_AK_TOGGLE
                && g_keys_advanced_key_states[index].toggle.latched != 0) {
                if (entry->layer != 0xFFu) {
                    return entry->layer;
                }
                break;
            }
        }
    }

    if ((key->flags & ANALOG_KEY_FLAG_PRESSED) != 0) {
        return (key->flags2 & ANALOG_KEY_FLAG2_LATCHED_LAYER_MASK) >> ANALOG_KEY_FLAG2_LATCHED_LAYER_SHIFT;
    }
    return (g_layout_layer_state & LAYOUT_STATE_ACTIVE_MASK) >> LAYOUT_STATE_ACTIVE_SHIFT;
}

/* Queues one event (enum keys_dks_event) for a dynamic-keystroke binding. The queue holds the
 * current step and up to three steps behind it; an event for a full queue is dropped. */
void keys_dks_queue_event(keys_dks_binding_queue_t *binding_queue, uint32_t event)
{
    uint8_t pending = binding_queue->pending;
    uint8_t last_step;   /* queue position of the last queued event */

    if (pending != 0) {
        if (pending == 3) {
            return;   /* queue full */
        }
        last_step = pending;
    } else if ((binding_queue->events & 3u) != KEYS_DKS_EVENT_NONE) {
        last_step = 0;
    } else {
        /* Empty queue: the event becomes the current step, its timer starts in the output stage. */
        uint8_t events = (uint8_t)(binding_queue->events | event);

        binding_queue->events = events;
        binding_queue->pending = 0;
        if ((events & 3u) != KEYS_DKS_EVENT_NONE) {
            binding_queue->timer = KEYS_DKS_TIMER_NOT_STARTED;
        }
        return;
    }

    /* The last queued event gets its release bit (a press becomes a tap) ... */
    uint8_t events = (uint8_t)(binding_queue->events | (KEYS_DKS_EVENT_RELEASE << (2u * last_step)));

    if (event == KEYS_DKS_EVENT_RELEASE) {
        binding_queue->events = events;
    } else {
        /* ... and a press or tap is appended behind it. */
        binding_queue->events = (uint8_t)(events | ((uint8_t)event << (2u * (last_step + 1u))));
        binding_queue->pending = (last_step + 1u) & 3u;
    }
}

/* Queues the events of one travel point for all four bindings of a dynamic keystroke:
 * `event_mask` carries 2 bits per binding (enum keys_dks_event), binding 0 in bits 1:0. */
void keys_dks_queue_events(keys_advanced_key_state_t *dks_state, uint32_t event_mask)
{
    for (uint32_t binding = 0; binding != KEYS_DKS_BINDINGS; binding++) {
        uint32_t event = ((int32_t)event_mask >> (2u * binding)) & 3u;

        if (event != KEYS_DKS_EVENT_NONE) {
            keys_dks_queue_event(&dks_state->dks.queue[binding], event);
        }
    }
}

/* Action word of a key code; "no key" is looked up as key code 0. */
static uint16_t keys_keycode_action(uint8_t keycode)
{
    if (keycode == LAYOUT_KEYCODE_NONE) {
        return k_layout_keycode_actions[0];
    }
    return k_layout_keycode_actions[keycode];
}

/* Counts the step timer of a dynamic-keystroke queue down. Returns true when the step is over
 * (timer left at 0); a timer that was not started is loaded with the tap length first. */
static bool keys_dks_step_elapsed(keys_dks_binding_queue_t *queue, const keys_pass_ctx_t *ctx)
{
    if (queue->timer == KEYS_DKS_TIMER_NOT_STARTED) {
        queue->timer = ctx->tap_ms & 0x3Fu;
        return false;
    }
    if (ctx->elapsed_ms < queue->timer) {
        queue->timer = (uint8_t)(queue->timer - ctx->elapsed_ms) & 0x3Fu;
        return false;
    }
    queue->timer = 0;
    return true;
}

/* Dynamic keystroke: plays the event queues of the four bindings. Returns 1 while the entry
 * still has output or the key is engaged. */
static uint8_t keys_run_dynamic_keystroke(keys_pass_ctx_t *ctx, const keys_key_ref_t *key_ref, uint32_t analog,
                                          const AdvancedKey *entry, keys_advanced_key_state_t *state)
{
    uint8_t key_id = key_ref->key_id;
    uint8_t active = 0;

    for (uint32_t binding = 0; binding != KEYS_DKS_BINDINGS; binding++) {
        keys_dks_binding_queue_t *queue = &state->dks.queue[binding];
        uint16_t action = keys_keycode_action(entry->kind.dynamic_keystroke.keycodes[binding]);

        if ((action >> LAYOUT_ACTION_CODE_SHIFT) == 0) {
            continue;
        }

        if ((action & LAYOUT_ACTION_ANALOG) != 0) {
            /* Analog bindings (gamepad axes) follow the travel directly. */
            keys_apply_key_action(ctx, action, key_id, entry->layer, (uint16_t)analog, 1, 1);
        } else {
            switch (queue->events & 3u) {
            case KEYS_DKS_EVENT_PRESS:
                /* Held until a release is queued; the timer only runs down. */
                keys_apply_key_action(ctx, action, key_id, entry->layer, (uint16_t)analog, 1, 1);
                (void)keys_dks_step_elapsed(queue, ctx);
                break;

            case KEYS_DKS_EVENT_RELEASE:
                /* Released gap, then the next queued step. */
                if (keys_dks_step_elapsed(queue, ctx)) {
                    uint8_t pending = queue->pending;

                    if (pending != 0) {
                        pending = (pending - 1u) & 3u;
                    } else if ((queue->events & 3u) == KEYS_DKS_EVENT_NONE) {
                        break;   /* cannot happen in this case */
                    }
                    queue->events >>= 2;
                    queue->pending = pending;
                    if ((queue->events & 3u) != KEYS_DKS_EVENT_NONE) {
                        queue->timer = KEYS_DKS_TIMER_NOT_STARTED;
                    }
                }
                break;

            case KEYS_DKS_EVENT_TAP:
                /* Pressed for the tap length, then the step turns into a released gap. */
                keys_apply_key_action(ctx, action, key_id, entry->layer, (uint16_t)analog, 1, 1);
                if (keys_dks_step_elapsed(queue, ctx)) {
                    uint8_t events = queue->events;

                    queue->events = events & 0xFEu;
                    if ((events & KEYS_DKS_EVENT_RELEASE) != 0) {
                        queue->timer = KEYS_DKS_TIMER_NOT_STARTED;
                    }
                }
                break;

            default:
                break;
            }
        }

        if (queue->events != 0) {
            active = 1;
        }
    }

    if (active == 0) {
        active = state->dks.phase != KEYS_DKS_IDLE;
    }
    return active;
}

/* Advances a mod-tap timer by the time since the previous pass; a timer that was not started
 * begins at 0. Returns the new value. */
static uint16_t keys_modtap_advance_timer(keys_advanced_key_state_t *state, const keys_pass_ctx_t *ctx)
{
    uint16_t timer_ms = state->modtap.timer_ms;

    if (timer_ms == KEYS_MODTAP_TIMER_IDLE) {
        timer_ms = 0;
    } else {
        timer_ms = (uint16_t)(timer_ms + ctx->elapsed_ms);
    }
    state->modtap.timer_ms = timer_ms;
    return timer_ms;
}

/* Mod-tap: hold decision by time, output of the hold key code or of the tap key code for the
 * tap length. Returns 1 while the entry is not idle. */
static uint8_t keys_run_mod_tap(keys_pass_ctx_t *ctx, const keys_key_ref_t *key_ref, uint32_t analog,
                                const AdvancedKey *entry, keys_advanced_key_state_t *state)
{
    const ModTap *mod_tap = &entry->kind.mod_tap;

    switch (state->modtap.state) {
    case KEYS_MODTAP_UNDECIDED: {
        uint32_t hold_ms = mod_tap->has_hold_time_10ms ? KEYS_MODTAP_HOLD_UNIT_MS * mod_tap->hold_time_10ms
                                                       : KEYS_MODTAP_DEFAULT_HOLD_MS;

        if (keys_modtap_advance_timer(state, ctx) < hold_ms) {
            return 1;
        }
        state->modtap.state = KEYS_MODTAP_HOLD;
    }
    /* fall through */
    case KEYS_MODTAP_HOLD:
        keys_apply_key_action(ctx, keys_keycode_action(mod_tap->hold_keycode), key_ref->key_id, entry->layer,
                              (uint16_t)analog, 1, 1);
        break;

    case KEYS_MODTAP_TAP: {
        uint32_t tap_ms;

        keys_apply_key_action(ctx, keys_keycode_action(mod_tap->tap_keycode), key_ref->key_id, entry->layer,
                              (uint16_t)analog, 1, 1);
        /* A Caps Lock tap is held longer so that the host does not miss it. */
        tap_ms = (mod_tap->tap_keycode == LAYOUT_KEYCODE_CAPS_LOCK) ? KEYS_MODTAP_CAPS_LOCK_TAP_MS : ctx->tap_ms;
        if (keys_modtap_advance_timer(state, ctx) >= tap_ms) {
            state->modtap.state = KEYS_MODTAP_IDLE;
            return 0;
        }
        break;
    }

    default:
        break;
    }

    return state->modtap.state != KEYS_MODTAP_IDLE;
}

/* Toggle key: output while held or latched (then with full travel). Returns the latch byte. */
static uint8_t keys_run_toggle_key(keys_pass_ctx_t *ctx, const keys_key_ref_t *key_ref, uint32_t analog,
                                   const AdvancedKey *entry, keys_advanced_key_state_t *state)
{
    uint16_t action;
    uint8_t pressed;
    uint16_t travel;

    if (state->toggle.pressed != 0) {
        state->toggle.hold_ms += ctx->elapsed_ms;
    }
    action = keys_keycode_action(entry->kind.toggle.keycode);
    pressed = state->toggle.pressed != 0 || state->toggle.latched != 0;
    travel = (state->toggle.latched != 0) ? KEYS_TRAVEL_MAX : (uint16_t)analog;
    keys_apply_key_action(ctx, action, key_ref->key_id, entry->layer, travel, 1, pressed);

    return state->toggle.latched;
}

/* Two-key bindings: stores the travel of this key in its half of the pair state and drops its
 * pressed flag on release; the resolution is done by keys_process_scan_pass. Returns 1 while
 * one of the two keys has travel. */
static uint8_t keys_update_key_pair_state(const keys_key_ref_t *key_ref, uint32_t analog, uint8_t second_key_id,
                                          keys_advanced_key_state_t *state)
{
    bool key_pressed = (key_ref->key_state->flags & ANALOG_KEY_FLAG_PRESSED) != 0;

    if (second_key_id == key_ref->key_id) {
        state->pair.key[1].travel = (uint16_t)analog;
        if (!key_pressed) {
            state->pair.key[1].pressed = 0;
        }
        if (state->pair.key[0].travel != 0) {
            return 1;
        }
    } else {
        state->pair.key[0].travel = (uint16_t)analog;
        if (!key_pressed) {
            state->pair.key[0].pressed = 0;
        }
        if (analog != 0) {
            return 1;
        }
    }
    return state->pair.key[1].travel != 0;
}

/* Output stage of the advanced keys bound on a key, run once per scan event of the key.
 * `current_index` is the entry bound on the key's current layer (ANALOG_NO_ADVANCED_KEY for
 * none); entries of other layers run only while they are still active. For each such entry the
 * state machine of its kind sends the key actions, a key-state entry is logged and the
 * "active" flag of the binding is updated. Two-key entries only record the travel here.
 * `analog` is the 12-bit travel of the key. */
void keys_process_advanced_keys(keys_pass_ctx_t *ctx, keys_key_ref_t *key_ref, uint32_t analog, uint32_t current_index)
{
    for (uint32_t layer = 0; layer != ANALOG_KEY_LAYERS; layer++) {
        analog_ak_slot_t *slot = &key_ref->key_state->ak_slot[layer];
        uint32_t index = slot->index;
        keys_advanced_key_state_t *state;
        AdvancedKey entry;
        uint16_t report_action;
        uint32_t report_pressed;
        uint8_t active;

        /* Bindings of other layers are only run while they still have work to do. */
        if (index == ANALOG_NO_ADVANCED_KEY || (slot->active == 0 && index != current_index)) {
            continue;
        }
        if (index >= g_keys_advanced_keys.entries_count) {
            slot->active = 0;
            continue;
        }

        entry = g_keys_advanced_keys.entries[index];
        state = &g_keys_advanced_key_states[index];
        report_action = (uint16_t)(((uint8_t)entry.which_kind << LAYOUT_ACTION_CODE_SHIFT) | KEYS_AK_REPORT_ACTION_KIND);

        switch ((uint8_t)entry.which_kind) {
        case KEYS_AK_DKS:
            active = keys_run_dynamic_keystroke(ctx, key_ref, analog, &entry, state);
            break;

        case KEYS_AK_MODTAP:
            active = keys_run_mod_tap(ctx, key_ref, analog, &entry, state);
            break;

        case KEYS_AK_TOGGLE:
            active = keys_run_toggle_key(ctx, key_ref, analog, &entry, state);
            break;

        case KEYS_AK_PAIR_DEEP:
            slot->active = keys_update_key_pair_state(key_ref, analog, entry.kind.depth_priority.second_key_id, state);
            continue;

        case KEYS_AK_PAIR_SOCD:
            slot->active = keys_update_key_pair_state(key_ref, analog, entry.kind.socd.second_key_id, state);
            continue;

        default:
            active = 0;
            break;
        }

        /* Key-state report: an active dynamic keystroke / mod-tap is logged as pressed, a
         * toggle key and an idle entry with the pressed state of the key itself. */
        if (entry.which_kind != KEYS_AK_TOGGLE && active != 0) {
            report_pressed = active;
        } else {
            report_pressed = (key_ref->key_state->flags & ANALOG_KEY_FLAG_PRESSED) != 0;
        }
        hid_keystate_report_add_key(report_action, analog, key_ref->key_id, report_pressed);
        slot->active = active;
    }
}

/* Validates an "ak2" message before it is stored or applied: drops unusable bottom-out points
 * of dynamic-keystroke entries. */
void keys_validate_advanced_keys(AdvancedKeys *ak_msg)
{
    uint32_t entry_count = ak_msg->entries_count;

    /* The index is 8 bit wide. */
    for (uint8_t i = 0; i < entry_count; i++) {
        AdvancedKey *entry = &ak_msg->entries[i];

        if ((uint8_t)entry->which_kind != KEYS_AK_DKS || !entry->kind.dynamic_keystroke.has_bottom_out_point) {
            continue;
        }
        /* A bottom-out point below 0.1 mm of travel is dropped (the default applies). */
        if (entry->kind.dynamic_keystroke.bottom_out_point
            < analog_get_key_travel_per_0p1mm(KEYS_KEY_ID_ROW(entry->key_id), KEYS_KEY_ID_COL(entry->key_id))) {
            entry->kind.dynamic_keystroke.has_bottom_out_point = false;
        }
    }
}

/* Stops the lighting overlay of every latched toggle key (the overlay id is the key id). */
void keys_stop_toggle_overlays(void)
{
    for (uint8_t i = 0; i < g_keys_advanced_keys.entries_count; i++) {
        const AdvancedKey *entry = &g_keys_advanced_keys.entries[i];

        if ((uint8_t)entry->which_kind == KEYS_AK_TOGGLE && g_keys_advanced_key_states[i].toggle.latched != 0) {
            rgb_overlay_stop(entry->key_id);
        }
    }
}

/* Applies entry `ak_index` of the advanced-key list: records the key position for every key
 * code of the entry that drives an indicator (layout_record_indicator_key) and binds the entry
 * to its key on its layer; a two-key entry is bound to both of its keys. */
void keys_apply_advanced_key_entry(uint8_t ak_index, const AdvancedKey *entry)
{
    uint32_t row = KEYS_KEY_ID_ROW(entry->key_id);
    uint32_t col = KEYS_KEY_ID_COL(entry->key_id);

    switch ((uint8_t)entry->which_kind) {
    case KEYS_AK_DKS:
        layout_record_indicator_key(entry->kind.dynamic_keystroke.keycodes[0], row, col);
        layout_record_indicator_key(entry->kind.dynamic_keystroke.keycodes[1], row, col);
        layout_record_indicator_key(entry->kind.dynamic_keystroke.keycodes[2], row, col);
        layout_record_indicator_key(entry->kind.dynamic_keystroke.keycodes[3], row, col);
        break;

    case KEYS_AK_MODTAP:
        layout_record_indicator_key(entry->kind.mod_tap.hold_keycode, row, col);
        layout_record_indicator_key(entry->kind.mod_tap.tap_keycode, row, col);
        break;

    case KEYS_AK_TOGGLE:
        layout_record_indicator_key(entry->kind.toggle.keycode, row, col);
        break;

    case KEYS_AK_PAIR_DEEP:
    case KEYS_AK_PAIR_SOCD: {
        /* The second key of the pair carries the same entry. */
        uint8_t second_key_id = entry->kind.depth_priority.second_key_id;

        keys_bind_advanced_key(KEYS_KEY_ID_ROW(second_key_id), KEYS_KEY_ID_COL(second_key_id), entry->layer, ak_index);
        break;
    }

    default:
        break;
    }

    keys_bind_advanced_key(row, col, entry->layer, ak_index);
}

/* Binds advanced-key entry `ak_index` to the key at row / col on `layer` and clears the
 * "active" flag of that binding. Positions without key are ignored. */
void keys_bind_advanced_key(uint32_t row, uint32_t col, uint32_t layer, uint8_t ak_index)
{
    analog_key_state_t *key;
    uint8_t pos;

    if (row > LAYOUT_ROWS - 1u || col > LAYOUT_COLS - 1u) {
        return;
    }
    pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];
    if (pos == ANALOG_NO_SCAN_POS) {
        return;
    }

    key = &g_analog_key_states[pos];
    /* The layer is not range-checked. */
    key->ak_slot[layer].active = 0;
    key->ak_slot[layer].index = ak_index;

    /* A key that is held while its binding changes is released by the key task: the pressed
     * bit moves into the "changed" bit. */
    if ((key->flags & (ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED)) != 0) {
        key->flags = (key->flags & (uint8_t)~ANALOG_KEY_FLAG_PRESSED) | ANALOG_KEY_FLAG_CHANGED;
    } else {
        key->flags &= (uint8_t)~(ANALOG_KEY_FLAG_PRESSED | ANALOG_KEY_FLAG_CHANGED);
    }
}
