/*
 * layout_key_actions.c - handlers of the special-key (Fn) actions and their dispatch table:
 * brightness keys, profile selection, layer keys, key disable toggle.
 *
 * Every handler gets the matrix position of the key, the layer of the binding and the new key
 * state, and returns 1.
 */
#include <string.h>

#include "hal/hal_irq.h"

#include "layout/layout.h"
#include "analog/analog_data.h"
#include "core/soft_timer.h"
#include "profile/profile_data.h"
#include "rgb/rgb_data.h"
#include "rgb/rgb_frame.h"
#include "rgb/signals/signals.h"

#define LAYOUT_BRIGHTNESS_REPEAT_UNITS   2u     /* soft-timer units of 25 ms: one step every 50 ms */
#define LAYOUT_PROFILE_BLINK_MS          1000u  /* length of the profile-switch blink */
#define LAYOUT_PROFILE_BLINK_PERIOD_MS   250u

/* Dispatch table of the special-key actions, index = enum layout_key_action. */
const layout_key_action_handler_t k_layout_key_action_handlers[LAYOUT_KEY_ACTION_COUNT] = {
    [LAYOUT_KEY_ACTION_INVALID]             = NULL,
    [LAYOUT_KEY_ACTION_BRIGHTNESS_UP]       = layout_key_action_brightness_up,
    [LAYOUT_KEY_ACTION_BRIGHTNESS_DOWN]     = layout_key_action_brightness_down,
    [LAYOUT_KEY_ACTION_PROFILE_1]           = layout_key_action_select_profile_1,
    [LAYOUT_KEY_ACTION_PROFILE_2]           = layout_key_action_select_profile_2,
    [LAYOUT_KEY_ACTION_PROFILE_3]           = layout_key_action_select_profile_3,
    [LAYOUT_KEY_ACTION_PROFILE_0]           = layout_key_action_select_profile_0,
    [LAYOUT_KEY_ACTION_NONE]                = layout_key_action_none,
    [LAYOUT_KEY_ACTION_TOGGLE_MODE_PROFILE] = layout_key_action_toggle_mode_profile,
    [LAYOUT_KEY_ACTION_LAYER1]              = layout_key_action_momentary_layer1,
    [LAYOUT_KEY_ACTION_TOGGLE_KEY_DISABLE]  = layout_key_action_toggle_key_disable,
    [LAYOUT_KEY_ACTION_LAYER_LOCK]          = layout_key_action_toggle_layer_lock,
    [LAYOUT_KEY_ACTION_LAYER2]              = layout_key_action_momentary_layer2,
    [LAYOUT_KEY_ACTION_LAYER3]              = layout_key_action_momentary_layer3,
    [LAYOUT_KEY_ACTION_NEXT_PROFILE]        = layout_key_action_next_profile,
};

/* ---- Brightness keys ---------------------------------------------------------------------------- */

/* Common body of the two brightness handlers: start (key pressed) or stop (key released) the
 * auto-repeat soft timer. */
static void layout_set_brightness_repeat(uint32_t pressed)
{
    uint32_t irq_state = hal_irq_save();

    if (pressed != 0u) {
        g_soft_timer_expired_mask &= (uint8_t)~(1u << SOFT_TIMER_BRIGHTNESS_REPEAT);
        g_soft_timer_active_mask |= (uint8_t)(1u << SOFT_TIMER_BRIGHTNESS_REPEAT);
        g_soft_timers[SOFT_TIMER_BRIGHTNESS_REPEAT] = (soft_timer_t){
            .remaining = LAYOUT_BRIGHTNESS_REPEAT_UNITS,
            .reload = LAYOUT_BRIGHTNESS_REPEAT_UNITS,
        };
    } else {
        g_soft_timer_active_mask &= (uint8_t)~(1u << SOFT_TIMER_BRIGHTNESS_REPEAT);
    }
    hal_irq_restore(irq_state);
}

/* Brightness up: one step every 50 ms while the key is held. */
int layout_key_action_brightness_up(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed)
{
    g_soft_timer_callbacks[SOFT_TIMER_BRIGHTNESS_REPEAT] = rgb_brightness_up_timer_cb;
    layout_set_brightness_repeat(pressed);
    return 1;
}

/* Brightness down: one step every 50 ms while the key is held. */
int layout_key_action_brightness_down(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed)
{
    g_soft_timer_callbacks[SOFT_TIMER_BRIGHTNESS_REPEAT] = rgb_brightness_down_timer_cb;
    layout_set_brightness_repeat(pressed);
    return 1;
}

/* Action without effect. */
int layout_key_action_none(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed)
{
    return 1;
}

/* ---- Profile keys ------------------------------------------------------------------------------- */

/* The key that triggered a profile switch produces no output until it has been released. */
static void layout_ignore_key_until_release(uint32_t row, uint32_t col)
{
    uint32_t scan_pos;

    if (row > LAYOUT_ROWS - 1u || col > LAYOUT_COLS - 1u) {
        return;
    }
    scan_pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];
    if (scan_pos != LAYOUT_NO_KEY) {
        g_analog_key_states[scan_pos].flags2 |= ANALOG_KEY_FLAG2_IGNORE_UNTIL_RELEASE;
    }
}

/* Toggle between built-in profile 0 and the profile that was active before (profile 1 if none is
 * remembered); with a selected "lp" profile, toggle between it and the profile to return to.
 * Only requests the switch (g_profile_switch_pending / g_profile_switch_target). */
int layout_key_action_toggle_mode_profile(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed)
{
    uint8_t target_index;
    uint8_t target_ns;

    if (pressed == 0u) {
        return 1;
    }
    layout_ignore_key_until_release(row, col);

    if (g_profile_lp_selected != 0u) {
        if (g_profile_active_id.ns == PROFILE_NS_LP) {
            target_index = g_profile_lp_return_id.index;
            target_ns = g_profile_lp_return_id.ns;
        } else {
            target_index = g_profile_lp_selected_id.index;
            target_ns = g_profile_lp_selected_id.ns;
        }
    } else if ((g_profile_active_id.index | g_profile_active_id.ns) == 0u) {
        /* On built-in profile 0: back to the remembered profile, or to profile 1 if none. */
        target_index = g_profile_toggle_target_id.index;
        target_ns = g_profile_toggle_target_id.ns;
        if ((target_index | target_ns) == 0u) {
            target_index = 1u;
        }
    } else if (g_profile_active_id.ns == PROFILE_NS_LP) {
        target_index = g_profile_lp_return_id.index;
        target_ns = g_profile_lp_return_id.ns;
    } else {
        /* On another built-in profile: remember it and go to profile 0. */
        g_profile_toggle_target_id.ns = g_profile_active_id.ns;
        g_profile_toggle_target_id.index = g_profile_active_id.index;
        target_index = 0u;
        target_ns = PROFILE_NS_BUILTIN;
    }

    g_profile_switch_target.index = target_index;
    g_profile_switch_pending = 1u;
    g_profile_switch_target.ns = target_ns;

    rgb_overlay_stop(RGB_OVERLAY_ID_PROFILE_SWITCH);
    return 1;
}

/* Request a switch to the next profile: the built-in profiles in order, then the selected "lp"
 * profile if there is one, then built-in profile 0 again. */
int layout_key_action_next_profile(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed)
{
    uint8_t next_index;
    uint8_t next_ns;

    if (pressed == 0u) {
        return 1;
    }
    layout_ignore_key_until_release(row, col);

    if (g_profile_active_id.ns == PROFILE_NS_LP) {
        next_index = 0u;
        next_ns = PROFILE_NS_BUILTIN;
    } else {
        uint8_t index = g_profile_active_id.index;

        next_index = (uint8_t)(index + 1u);
        next_ns = PROFILE_NS_BUILTIN;
        if (g_profile_counts[PROFILE_COUNTS_BUILTIN] <= next_index) {
            /* Past the last built-in profile: the selected "lp" profile if there is one, else wrap. */
            if (g_profile_counts[PROFILE_COUNTS_LP] != 0u && g_profile_lp_selected != 0u) {
                next_index = g_profile_lp_selected_id.index;
                next_ns = g_profile_lp_selected_id.ns;
            } else {
                next_index = 0u;
            }
        }
        if (next_index == index && g_profile_active_id.ns == next_ns) {
            return 1;
        }
    }

    g_profile_switch_pending = 1u;
    g_profile_switch_target.index = next_index;
    g_profile_switch_target.ns = next_ns;
    return 1;
}

/* Request a switch to built-in profile `profile_index` (ignored if it does not exist or is
 * already active) and blink the key at row / col for one second. */
void layout_request_profile_switch(uint32_t row, uint32_t col, uint32_t profile_index)
{
    rgb_overlay_params_t params;

    if (g_profile_counts[PROFILE_COUNTS_BUILTIN] <= profile_index) {
        return;
    }
    if (g_profile_active_id.index == profile_index && g_profile_active_id.ns == PROFILE_NS_BUILTIN) {
        return;
    }

    g_profile_switch_pending = 1u;
    g_profile_switch_target.index = (uint8_t)profile_index;
    g_profile_switch_target.ns = PROFILE_NS_BUILTIN;
    layout_ignore_key_until_release(row, col);

    /* Blink the pressed key white / black for one second. */
    memset(&params, 0, sizeof(params));
    params.type = RGB_OVERLAY_BLINK_KEY;
    params.u.blink_key.color_a = (rgb_argb_t){ .b = 0xFFu, .g = 0xFFu, .r = 0xFFu, .a = 0xFFu };
    params.u.blink_key.color_b.a = 0xFFu;
    params.u.blink_key.key_pos = (uint8_t)((col & 0x1Fu) | (row << 5));
    params.u.blink_key.period_ms = LAYOUT_PROFILE_BLINK_PERIOD_MS;
    if (g_rgb_sdk_mode_active == 0u) {
        rgb_overlay_start(RGB_OVERLAY_ID_PROFILE_SWITCH, LAYOUT_PROFILE_BLINK_MS, &params);
    }
}

/* Press of the "profile 1" key. */
int layout_key_action_select_profile_1_pressed(uint32_t row, uint32_t col)
{
    layout_request_profile_switch(row, col, 1u);
    return 1;
}

/* Select built-in profile 1 on press; release does nothing. */
int layout_key_action_select_profile_1(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed)
{
    if (pressed != 0u) {
        return layout_key_action_select_profile_1_pressed(row, col);
    }
    return 1;
}

/* Press of the "profile 2" key. */
int layout_key_action_select_profile_2_pressed(uint32_t row, uint32_t col)
{
    layout_request_profile_switch(row, col, 2u);
    return 1;
}

/* Select built-in profile 2 on press; release does nothing. */
int layout_key_action_select_profile_2(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed)
{
    if (pressed != 0u) {
        return layout_key_action_select_profile_2_pressed(row, col);
    }
    return 1;
}

/* Press of the "profile 3" key. */
int layout_key_action_select_profile_3_pressed(uint32_t row, uint32_t col)
{
    layout_request_profile_switch(row, col, 3u);
    return 1;
}

/* Select built-in profile 3 on press; release does nothing. */
int layout_key_action_select_profile_3(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed)
{
    if (pressed != 0u) {
        return layout_key_action_select_profile_3_pressed(row, col);
    }
    return 1;
}

/* Press of the "profile 0" key. */
int layout_key_action_select_profile_0_pressed(uint32_t row, uint32_t col)
{
    layout_request_profile_switch(row, col, 0u);
    return 1;
}

/* Select built-in profile 0 on press; release does nothing. */
int layout_key_action_select_profile_0(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed)
{
    if (pressed != 0u) {
        return layout_key_action_select_profile_0_pressed(row, col);
    }
    return 1;
}

/* ---- Key disable toggle --------------------------------------------------------------------------- */

/* Toggle the "disabled" state of the key at row / col on press and mark a disabled key with a
 * black overlay. */
int layout_key_action_toggle_key_disable(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed)
{
    analog_key_state_t *key;
    uint32_t scan_pos;
    uint8_t key_pos;

    if (pressed == 0u || row > LAYOUT_ROWS - 1u || col > LAYOUT_COLS - 1u) {
        return 1;
    }
    scan_pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * row + col];
    if (scan_pos == LAYOUT_NO_KEY) {
        return 1;
    }

    key = &g_analog_key_states[scan_pos];
    key->disabled = !key->disabled;
    key_pos = (uint8_t)((col & 0x1Fu) | ((row & 7u) << 5));

    if (key->disabled) {
        /* Mark the disabled key: solid black overlay whose id is the key position. */
        rgb_overlay_params_t params;

        memset(&params, 0, sizeof(params));
        params.type = RGB_OVERLAY_SOLID_KEY;
        params.u.solid_key.color.a = 0xFFu;
        params.u.solid_key.key_pos = key_pos;
        if (key_pos != 0u && g_rgb_sdk_mode_active == 0u) {
            rgb_overlay_start(key_pos, RGB_OVERLAY_DURATION_FOREVER, &params);
        }
    } else {
        rgb_overlay_stop(key_pos);
    }
    return 1;
}

/* ---- Layer keys ----------------------------------------------------------------------------------- */

/* Tell the RGB code which layer is active (key colours of the layer, signal "active layer"). */
static void layout_publish_active_layer(uint32_t active_layer)
{
    if (g_rgb_sdk_mode_active == 0u && g_rgb_shown_color_layer != active_layer) {
        g_rgb_layer_switch_pending = 1u;
        g_rgb_requested_layer = (uint8_t)active_layer;
    }
    signals_publish_if_changed(RGB_SIGNAL_ACTIVE_LAYER, (float)active_layer);
}

/* Lock `layer` as the active layer on press; locking the locked layer again returns to layer 0. */
int layout_key_action_toggle_layer_lock(uint32_t row, uint32_t col, uint32_t layer, uint32_t pressed)
{
    if (pressed == 0u || layer > 3u) {
        return 1;
    }
    if (((g_layout_layer_state & LAYOUT_STATE_LOCKED_MASK) >> LAYOUT_STATE_LOCKED_SHIFT) == layer) {
        layer = 0u; /* locking the locked layer again unlocks */
    }
    g_layout_layer_state = (uint8_t)((g_layout_layer_state & ~(LAYOUT_STATE_ACTIVE_MASK | LAYOUT_STATE_LOCKED_MASK))
                                     | ((layer & 3u) << LAYOUT_STATE_LOCKED_SHIFT)
                                     | ((layer & 3u) << LAYOUT_STATE_ACTIVE_SHIFT));
    rgb_update_lock_indicators();
    layout_publish_active_layer((g_layout_layer_state & LAYOUT_STATE_ACTIVE_MASK) >> LAYOUT_STATE_ACTIVE_SHIFT);
    return 1;
}

/* Common body of the three momentary layer handlers: while held, the key shows `layer`, or the
 * base layer if `layer` is the locked one; on release the locked layer becomes active again. */
static int layout_momentary_layer(uint32_t layer, uint32_t pressed)
{
    uint32_t locked = (g_layout_layer_state & LAYOUT_STATE_LOCKED_MASK) >> LAYOUT_STATE_LOCKED_SHIFT;
    uint32_t active;

    if (pressed != 0u) {
        active = locked == layer ? 0u : layer;
        g_layout_layer_state =
            (uint8_t)((g_layout_layer_state & ~LAYOUT_STATE_ACTIVE_MASK) | (active << LAYOUT_STATE_ACTIVE_SHIFT));
    } else {
        active = (g_layout_layer_state & LAYOUT_STATE_ACTIVE_MASK) >> LAYOUT_STATE_ACTIVE_SHIFT;
        if (active == layer || locked == layer) {
            active = locked;
            g_layout_layer_state =
                (uint8_t)((g_layout_layer_state & ~LAYOUT_STATE_ACTIVE_MASK) | (active << LAYOUT_STATE_ACTIVE_SHIFT));
        }
    }
    layout_publish_active_layer(active);
    return 1;
}

/* Momentary layer 1 (Fn). */
int layout_key_action_momentary_layer1(uint32_t row, uint32_t col, uint32_t arg, uint32_t pressed)
{
    return layout_momentary_layer(1u, pressed);
}

/* Momentary layer 2. */
int layout_key_action_momentary_layer2(uint32_t row, uint32_t col, uint32_t arg, uint32_t pressed)
{
    return layout_momentary_layer(2u, pressed);
}

/* Momentary layer 3. */
int layout_key_action_momentary_layer3(uint32_t row, uint32_t col, uint32_t arg, uint32_t pressed)
{
    return layout_momentary_layer(3u, pressed);
}
