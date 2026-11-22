/*
 * keys_scan_pass.c - the key pipeline: one pass per completed hall-sensor scan.
 *
 * keys_process_scan_pass turns the scan events of one cycle into key actions and HID reports.
 * It is one long pass, split into the steps S0..S14. This file holds the pass itself and the
 * per-event steps that apply to every key:
 *
 *   S0  keys_reset_pass_outputs    clear everything the pass rebuilds
 *   S1  keys_take_pass_time        milliseconds since the previous pass, tap length
 *   S2  event loop                 (in keys_process_scan_pass)
 *   S3  keys_process_event         layer and advanced-key lookup, runs S4..S7 for one event
 *   S5  keys_handle_key_edge       press / release edge work
 *   S6  keys_update_key_common     RGB travel feed, last pressed key, deferred output
 *   S7  keys_emit_key_output       advanced-key output or keymap action
 *   S12 keys_dispatch_fn_actions   special-key (Fn) handlers
 *   S14 keys_publish_key_activity  lighting idle timer; published values "key pressed", "analog level"
 *
 * The other steps live in keys_scan_pass_advanced.c (S4: input stages of the advanced keys),
 * keys_scan_pass_pairs.c (S8: two-key resolvers) and keys_scan_pass_reports.c (S9..S11, S13:
 * gamepad, keyboard, extra and key-state reports).
 *
 * All outputs are rebuilt from zero in every pass, so the pass relies on the scan code queueing
 * one event per pass for every key that is not at rest.
 */
#include <string.h>

#include "tusb.h"

#include "keys/keys_scan_pass.h"
#include "keys/keys_scan_pass_internal.h"
#include "keys/keys.h"
#include "adc_scan/adc_scan.h"
#include "hal/hal_irq.h"
#include "hid/hid.h"
#include "layout/layout.h"
#include "rgb/rgb_types.h"
#include "rgb/rgb_data.h"
#include "rgb/rgb_effects.h"
#include "rgb/rgb_frame.h"
#include "rgb/signals/signals.h"
#include "core/clock.h"
#include "usb/usb_types.h"
#include "usb/usb_data.h"
#include "tables/keys_tables.h"

/* The special-key handler table (15 pointers, entry 0 NULL), defined in
 * layout/layout_key_actions.c and also declared in layout/layout.h. */
extern const layout_key_action_handler_t k_layout_key_action_handlers[LAYOUT_KEY_ACTION_COUNT];

/* ---- S0 ----------------------------------------------------------------------------------------- */

/* Clears everything the pass rebuilds: the last pressed key, the keyboard, extra and key-state
 * reports under construction and the pass context. */
static void keys_reset_pass_outputs(keys_scan_pass_t *pass)
{
    g_keys_last_pressed_key_id = k_layout_no_key_pos;

    memset(&g_hid_key_state_report, 0, sizeof(g_hid_key_state_report));
    /* The report id byte keeps its value: an "all released" report carries the id of the last use. */
    memset(g_hid_extra_report.data, 0, sizeof(g_hid_extra_report.data));
    g_hid_consumer_usage_count = 0;
    memset(&g_hid_nkro_report, 0, sizeof(g_hid_nkro_report));
    memset(&g_hid_boot_kbd_report, 0, sizeof(g_hid_boot_kbd_report));
    g_hid_key_state_report_count = 0;

    memset(&pass->ctx, 0, sizeof(pass->ctx));
}

/* ---- S1 ----------------------------------------------------------------------------------------- */

/* Stores the time since the previous pass (ms, 16 bit) and the tap length of the profile's
 * preset in the pass context and restarts the stamp. Interrupts are masked meanwhile so that
 * the uptime and the preset are read consistently. */
static void keys_take_pass_time(keys_scan_pass_t *pass)
{
    uint8_t preset;
    uint16_t previous_stamp;
    uint32_t irq_state = hal_irq_save();

    preset = g_keys_tap_duration_preset;
    previous_stamp = (uint16_t)g_keys_task_stamp_ms;
    g_keys_task_stamp_ms = g_clock_uptime_ms;
    pass->ctx.elapsed_ms = (uint16_t)(g_clock_uptime_ms - previous_stamp);
    pass->ctx.tap_ms = (preset <= KEYS_TAP_PRESET_COUNT - 1u) ? k_keys_tap_duration_ms[preset]
                                                              : KEYS_TAP_MS_FALLBACK;
    hal_irq_restore(irq_state);
}

/* ---- S5 ----------------------------------------------------------------------------------------- */

/* The released key carries an advanced key that does not produce the "disengaged" RGB event on
 * its own: its index is out of range or it is not a mod-tap, toggle or two-key binding. */
static bool keys_release_needs_reset_event(const keys_key_ref_t *ref)
{
    uint32_t ak_index = ref->key_state->ak_slot[keys_get_binding_layer(ref)].index;

    if (g_keys_advanced_keys.entries_count <= ak_index)
        return true;
    return (uint8_t)(keys_pass_ak_kind(&g_keys_advanced_keys.entries[ak_index]) - KEYS_AK_MODTAP) >
           KEYS_AK_PAIR_SOCD - KEYS_AK_MODTAP;
}

/* Press / release edge of one key (record flag `changed`, set by keys_update_press_state or by the
 * dynamic keystroke stage). ak_index and layer are the values of S3; returns the layer to use for
 * the output, which a press looks up again because the latch below can change it. */
static uint32_t keys_handle_key_edge(keys_scan_pass_t *pass, keys_key_ref_t *ref, uint32_t ak_index, uint32_t layer)
{
    analog_key_state_t *key = ref->key_state;
    uint32_t row = KEYS_KEY_ID_ROW(ref->key_id);
    uint32_t col = KEYS_KEY_ID_COL(ref->key_id);

    if (key->flags & ANALOG_KEY_FLAG_PRESSED) {
        /* Another key went down: decide the mod-tap keys that are still waiting. */
        keys_pass_modtap_resolve_pending(pass, ref);

        /* The key stays on the layer that is active now until it is released. */
        key->flags2 = (uint8_t)((key->flags2 & ~ANALOG_KEY_FLAG2_LATCHED_LAYER_MASK) |
                                (((g_layout_layer_state & LAYOUT_STATE_ACTIVE_MASK) >> LAYOUT_STATE_ACTIVE_SHIFT)
                                 << ANALOG_KEY_FLAG2_LATCHED_LAYER_SHIFT));
        if (!g_rgb_sdk_mode_active)
            rgb_effects_handle_key_event(row, col, RGB_KEY_EVENT_PRESSED);

        signals_key_rate_note_press();

        layer = keys_get_binding_layer(ref);
    } else {
        uint32_t rapid_trigger;

        key->flags2 = (uint8_t)((key->flags2 & ~ANALOG_KEY_FLAG2_LATCHED_LAYER_MASK) |
                                (((g_layout_layer_state & LAYOUT_STATE_LOCKED_MASK) >> LAYOUT_STATE_LOCKED_SHIFT)
                                 << ANALOG_KEY_FLAG2_LATCHED_LAYER_SHIFT));
        if (!g_rgb_sdk_mode_active)
            rgb_effects_handle_key_event(row, col, RGB_KEY_EVENT_RELEASED);

        /* With rapid trigger keys_update_press_state sends the "disengaged" event itself when the
         * key leaves the rapid-trigger zone. */
        if (key->flags & ANALOG_KEY_FLAG_RT_OVERRIDE)
            rapid_trigger = (key->flags & ANALOG_KEY_FLAG_RT_ON) != 0;
        else
            rapid_trigger = g_keys_rapid_trigger_enabled;
        if (rapid_trigger == 0 ||
            (ak_index != ANALOG_NO_ADVANCED_KEY && keys_release_needs_reset_event(ref))) {
            if (!g_rgb_sdk_mode_active)
                rgb_effects_handle_key_event(row, col, RGB_KEY_EVENT_RESET);
        }
    }

    key->flags &= (uint8_t)~ANALOG_KEY_FLAG_CHANGED;
    return layer;
}

/* ---- S6 ----------------------------------------------------------------------------------------- */

/* Work done for every event. Returns true when the output of this event is held back: the key
 * interrupted a pending mod-tap in an earlier pass step (record flag `defer_output`), so the host
 * sees the mod-tap's key first. The flag is consumed here, except for two-key bindings, whose
 * resolver (S8) consumes it. */
static bool keys_update_key_common(keys_key_ref_t *ref, uint32_t travel, uint32_t ak_index)
{
    analog_key_state_t *key = ref->key_state;
    bool pair_owns_flag = false;

    if (travel == 0)
        key->flags2 &= (uint8_t)~ANALOG_KEY_FLAG2_IGNORE_UNTIL_RELEASE;

    rgb_effects_feed_key_travel(KEYS_KEY_ID_ROW(ref->key_id), KEYS_KEY_ID_COL(ref->key_id), (uint16_t)travel);

    if (key->flags & ANALOG_KEY_FLAG_PRESSED)
        g_keys_last_pressed_key_id = ref->key_id;

    if (!(key->flags & ANALOG_KEY_FLAG_DEFER_OUTPUT))
        return false;

    if (ak_index != ANALOG_NO_ADVANCED_KEY) {
        uint32_t current_index = key->ak_slot[keys_get_binding_layer(ref)].index;

        if (current_index < g_keys_advanced_keys.entries_count) {
            uint8_t kind = keys_pass_ak_kind(&g_keys_advanced_keys.entries[current_index]);

            pair_owns_flag = (uint8_t)(kind - KEYS_AK_PAIR_DEEP) <= KEYS_AK_PAIR_SOCD - KEYS_AK_PAIR_DEEP;
        }
    }
    if (!pair_owns_flag)
        key->flags &= (uint8_t)~ANALOG_KEY_FLAG_DEFER_OUTPUT;
    return true;
}

/* ---- S7 ----------------------------------------------------------------------------------------- */

/* An advanced key bound on any layer of the key still has work to do (queued dynamic keystroke
 * events, a tap in progress, a latched toggle). */
static bool keys_has_active_advanced_key(const analog_key_state_t *key)
{
    for (uint32_t layer = 0; layer < ANALOG_KEY_LAYERS; layer++) {
        if (key->ak_slot[layer].index != ANALOG_NO_ADVANCED_KEY && key->ak_slot[layer].active != 0)
            return true;
    }
    return false;
}

/* Output of one event: the advanced key bound on the key's layer, or the keymap action of a plain
 * key plus its direct gamepad binding. `layer` is the layer of S3 / S5. */
static void keys_emit_key_output(keys_scan_pass_t *pass, keys_key_ref_t *ref, uint32_t travel, uint32_t layer)
{
    analog_key_state_t *key = ref->key_state;
    uint32_t ak_index = key->ak_slot[keys_get_binding_layer(ref)].index;
    uint16_t action;
    uint8_t flags;
    uint8_t pressed;

    if (ak_index != ANALOG_NO_ADVANCED_KEY) {
        keys_process_advanced_keys(&pass->ctx, ref, travel, ak_index);
        return;
    }

    /* Plain key on this layer; advanced keys of its other layers may still be running. */
    if (keys_has_active_advanced_key(key))
        keys_process_advanced_keys(&pass->ctx, ref, travel, ANALOG_NO_ADVANCED_KEY);

    action = keys_pass_layer_action(key, layer);

    flags = key->flags;
    pressed = 0;
    if (flags & ANALOG_KEY_FLAG_PRESSED) {
        /* A disabled key only passes the action that toggles the disable state. */
        bool enabled = !(flags & ANALOG_KEY_FLAG_DISABLED) ||
                       ((action & LAYOUT_ACTION_KIND_MASK) == LAYOUT_ACTION_SPECIAL &&
                        (action >> LAYOUT_ACTION_CODE_SHIFT) == LAYOUT_KEY_ACTION_TOGGLE_KEY_DISABLE);
        /* "Gamepad keys exclusive": a key with a gamepad binding gives no layer-0 key action
         * while a gamepad interface exists. */
        bool not_gamepad_only = g_hid_gamepad_keys_exclusive == 0 ||
                                key->gamepad_binding == 0 ||
                                (g_usb_link_flags & USB_LINK_MODE_MASK) == USB_MODE_NO_GAMEPAD ||
                                layer != 0;

        pressed = enabled && not_gamepad_only && !(key->flags2 & ANALOG_KEY_FLAG2_IGNORE_UNTIL_RELEASE);
    }

    keys_apply_key_action(&pass->ctx, action, ref->key_id, (uint8_t)layer, (uint16_t)travel, 0, pressed);

    if ((g_usb_link_flags & USB_LINK_MODE_MASK) != USB_MODE_NO_GAMEPAD && key->gamepad_binding != 0 && layer == 0) {
        hid_gamepad_apply_binding(&pass->ctx.gamepad, key->gamepad_binding, travel,
                                  (key->flags & ANALOG_KEY_FLAG_PRESSED) != 0);
    }
}

/* ---- S3 ----------------------------------------------------------------------------------------- */

/* One scan event: key id, key record, travel 0..4095. Looks up the layer and the advanced key
 * of the key, updates its press state and runs S4..S7 for it. */
static void keys_process_event(keys_scan_pass_t *pass, const adc_scan_event_t *event)
{
    analog_key_state_t *key = event->key_state;
    keys_key_ref_t ref;
    uint32_t layer;
    uint32_t travel;
    uint32_t ak_index;

    ref.key_id = event->key_id;
    ref.key_state = key;
    layer = keys_get_binding_layer(&ref);
    travel = event->travel;
    ak_index = key->ak_slot[layer].index;

    if (ak_index == ANALOG_NO_ADVANCED_KEY) {
        /* Plain key: actuation point / rapid trigger decision. */
        keys_update_press_state(&ref, event->travel);
    } else if (ak_index < g_keys_advanced_keys.entries_count) {
        keys_pass_advanced_key_input(pass, &ref, travel, ak_index);  /* S4 */
    }
    /* An index beyond the list: the press state of the key is not updated at all. */

    if (key->flags & ANALOG_KEY_FLAG_CHANGED)
        layer = keys_handle_key_edge(pass, &ref, ak_index, layer);  /* S5 */

    if (!keys_update_key_common(&ref, travel, ak_index))            /* S6 */
        keys_emit_key_output(pass, &ref, travel, layer);            /* S7 */

    /* Deepest travel of the pass, for the RGB signal "analog level". */
    if (pass->deepest_travel < event->travel)
        pass->deepest_travel = event->travel;
}

/* ---- S12 ---------------------------------------------------------------------------------------- */

/* Calls the handler of every special-key action whose pressed state differs from the last
 * dispatched one. keys_apply_key_action normally takes the press edge itself (it stores the state
 * and calls the handler), so this mostly delivers releases: the request of an action nobody holds
 * is all zero and the handler runs with (0, 0, 0, 0). */
static void keys_dispatch_fn_actions(keys_scan_pass_t *pass)
{
    for (uint32_t action = 0; action < KEYS_FN_ACTION_COUNT; action++) {
        keys_fn_action_state_t request = pass->ctx.fn[action];

        if (g_keys_fn_action_states[action].pressed == request.pressed)
            continue;
        g_keys_fn_action_states[action] = request;
        /* KNOWN ISSUE: handler 0 is NULL and is called unchecked when the pressed state of
         * action 0 changes. Unreachable with the shipped key code table: no key code maps to
         * action 0. Matches stock firmware 2.14.1. */
        /* The fourth argument of every call is the pressed bit of the request. Action 7
         * (layout_key_action_none) is an empty function. */
        k_layout_key_action_handlers[action](KEYS_KEY_ID_ROW(request.key_id), KEYS_KEY_ID_COL(request.key_id),
                                             request.layer, request.pressed);
    }
}

/* ---- S14 ---------------------------------------------------------------------------------------- */

/* A pressed key restarts the lighting idle timer. Publishes the signals "key pressed" (1.0
 * while a key is pressed) and "analog level" (deepest travel of the pass scaled to 0..255). */
static void keys_publish_key_activity(const keys_scan_pass_t *pass)
{
    float key_pressed = 0.0f;

    if (g_keys_last_pressed_key_id != KEYS_KEY_ID_NONE) {
        rgb_reset_idle_timer();
        /* The key id is tested again after the call. */
        key_pressed = (g_keys_last_pressed_key_id != KEYS_KEY_ID_NONE) ? 1.0f : 0.0f;
    }
    signals_publish_if_changed(RGB_SIGNAL_KEY_PRESSED, key_pressed);
    signals_publish_if_changed(RGB_SIGNAL_ANALOG_LEVEL, (float)pass->deepest_travel / 4095.0f * 255.0f);
}

/* ---- The pass ----------------------------------------------------------------------------------- */

/* Key processing pass, called by startup_run_active_task after every scan and by
 * startup_run_suspended_task about every 100 ms while USB is suspended. */
void keys_process_scan_pass(void)
{
    keys_scan_pass_t pass;
    adc_scan_event_t event;
    int result;

    keys_reset_pass_outputs(&pass);  /* S0 */
    keys_take_pass_time(&pass);      /* S1 */

    /* S2: take the events of this scan cycle. While the queue is empty and the scan is still
     * running the USB task runs (busy wait for the ADC interrupt). */
    memset(&event, 0, sizeof(event));
    result = adc_scan_event_pop(&event);
    pass.deepest_travel = 0;
    pass.dks_bottom_travel = 0;
    while (result != ADC_SCAN_POP_CYCLE_END) {
        if (result == ADC_SCAN_POP_EVENT)
            keys_process_event(&pass, &event);  /* S3..S7 */
        else if (result == ADC_SCAN_POP_EMPTY && tud_inited())
            tud_task();
        result = adc_scan_event_pop(&event);
    }

    keys_pass_resolve_two_key_pairs(&pass);  /* S8 */
    keys_pass_send_gamepad_report(&pass);    /* S9 */

    if (g_usb_link_flags & USB_LINK_KEY_REPORTS) {
        /* S10. A send attempt runs USB code, so the flag is tested again after one; S11..S13
         * are skipped if key reports were switched off meanwhile. */
        bool send_attempted = keys_pass_send_keyboard_report();

        if (!send_attempted || (g_usb_link_flags & USB_LINK_KEY_REPORTS)) {
            keys_pass_send_extra_report();       /* S11 */
            keys_dispatch_fn_actions(&pass);     /* S12 */
            keys_pass_send_key_state_report();   /* S13 */
        }
    }

    keys_publish_key_activity(&pass);  /* S14 */
}
