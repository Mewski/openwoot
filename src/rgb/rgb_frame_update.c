/*
 * rgb_frame_update.c - the RGB task step (rgb_update_frame) and the functions that switch
 * the LED output on, wake the driver and end idle dimming.
 *
 * rgb_update_frame runs on every main-loop pass while the LED driver is enabled. A call
 * either renders one frame (at most every 16 ms unless forced) or sends one dirty frame page
 * to the LED driver chip. The frame holds linear values; gamma and brightness are applied when a
 * page is sent.
 */
#include "rgb/rgb_frame.h"

#include <string.h>

#include "core/clock.h"
#include "core/run_state.h"
#include "drivers/led_matrix.h"
#include "hal/hal_irq.h"
#include "profile/profile_data.h"
#include "rgb/rgb_effect_layers.h"
#include "rgb/lightbar/lightbar.h"
#include "storage/storage.h"
#include "tables/rgb_color_tables.h"
#include "usb/usb_data.h"
#include "usb/usb_types.h"

#define RGB_IDLE_MS_PER_SECOND     1000
#define RGB_IDLE_SECONDS_MAX       60   /* the seconds counter wraps when it exceeds this value */
#define RGB_IDLE_MINUTES_MAX       99   /* minutes stop counting above this value */
#define RGB_IDLE_DIM_FADE_MS       2000
#define RGB_IDLE_OFF_FADE_MS       3000
#define RGB_IDLE_WAKE_FADE_MS      500
#define RGB_DRIVER_WAKE_FADE_MS    250

/* Derives g_rgb_driver_enabled from the USB link state and flags a change for the main-loop tasks. */
void rgb_update_output_enable(void)
{
    /* LEDs are on only while USB is configured and not suspended. */
    if ((g_usb_link_flags & USB_LINK_CONFIGURED) == 0 || (g_usb_link_flags & USB_LINK_SUSPENDED) != 0) {
        if (g_rgb_driver_enabled != 0) {
            g_rgb_driver_enabled = 0;
            g_rgb_driver_state_changed = 1;
        }
    } else if (g_rgb_driver_enabled == 0) {
        g_rgb_driver_enabled = 1;
        g_rgb_driver_state_changed = 1;
    }
}

/* Prepares a brightness fade overlay record (type 2). */
static void rgb_fade_params(rgb_overlay_params_t *params, uint8_t from, uint8_t to, uint16_t fade_ms)
{
    memset(params, 0, sizeof(*params));
    params->type = RGB_OVERLAY_FADE;
    params->u.fade.from = from;
    params->u.fade.to = to;
    params->u.fade.fade_ms = fade_ms;
}

/*
 * Takes the LED driver chip out of shutdown and starts a 250 ms fade-in from black.
 */
void rgb_wake_led_driver(void)
{
    rgb_overlay_params_t params;

    led_matrix_set_running(true);

    g_rgb_effect_stamp_ms = g_clock_uptime_ms;

    /* Fade in from black. */
    rgb_fade_params(&params, 0, RGB_FADE_LEVEL_FULL, RGB_DRIVER_WAKE_FADE_MS);
    if (g_rgb_sdk_mode_active == 0) {
        rgb_overlay_start(RGB_OVERLAY_ID_WAKE_FADE, RGB_DRIVER_WAKE_FADE_MS, &params);
    }
}

/*
 * Key activity: restarts the idle timer and, if the LEDs were dimmed or off, fades them back
 * to full brightness within 500 ms.
 */
void rgb_reset_idle_timer(void)
{
    if ((g_rgb_idle_flags & (RGB_IDLE_FLAG_DIMMED | RGB_IDLE_FLAG_OFF)) != 0) {
        rgb_overlay_params_t params;

        /* Fade back to full brightness from where the idle state left the LEDs. */
        rgb_fade_params(&params, (g_rgb_idle_flags & RGB_IDLE_FLAG_OFF) != 0 ? 0 : RGB_FADE_LEVEL_DIMMED,
                        RGB_FADE_LEVEL_FULL, RGB_IDLE_WAKE_FADE_MS);
        if (g_rgb_sdk_mode_active == 0) {
            rgb_overlay_start(RGB_OVERLAY_ID_IDLE_FADE, RGB_IDLE_WAKE_FADE_MS, &params);
        }
    }
    g_rgb_idle_timer.ms = 0;
    g_rgb_idle_timer.seconds = 0;
    g_rgb_idle_timer.minutes = 0;
    g_rgb_idle_flags &= (uint8_t)~(RGB_IDLE_FLAG_DIMMED | RGB_IDLE_FLAG_OFF);
}

/* Paints a key matrix RGB565 colour map into the key LEDs of the main frame. */
static void rgb_paint_layer_colors(const uint16_t colors[RGB_MATRIX_ROWS][RGB_MATRIX_COLS])
{
    for (int row = 0; row != RGB_MATRIX_ROWS; ++row) {
        for (int col = 0; col != RGB_MATRIX_COLS; ++col) {
            uint32_t led_id = g_rgb_key_to_led[row * RGB_MATRIX_COLS + col];
            uint32_t argb = rgb_convert_rgb565_to_argb(colors[row][col]);

            rgb_frame_set_led(&g_rgb_frame_main, led_id, (uint8_t)(argb >> 16), (uint8_t)(argb >> 8), (uint8_t)argb);
        }
    }
}

/*
 * Pending layer colour map switch: shows the map of g_rgb_requested_layer (cached map, else
 * file r<layer> of the active profile) or returns to the base colours.
 */
static void rgb_apply_layer_colors(void)
{
    uint16_t file_colors[RGB_MATRIX_ROWS][RGB_MATRIX_COLS];
    uint8_t layer;

    if (g_rgb_layer_switch_pending == 0) {
        return;
    }
    g_rgb_layer_switch_pending = 0;

    layer = g_rgb_requested_layer;
    if (layer != 0) {
        if (g_rgb_layer_color_count >= layer) {
            g_rgb_shown_color_layer = layer;
            g_rgb_shown_layer_colors = &g_rgb_layer_colors[layer - 1][0][0];
            rgb_paint_layer_colors(g_rgb_layer_colors[layer - 1]);
            g_rgb_force_render = 1;
            lightbar_request_redraw();
            return;
        }
        if (storage_read_layer_colors(g_profile_active_id.raw, g_rgb_requested_layer, file_colors) == 1) {
            g_rgb_shown_color_layer = g_rgb_requested_layer;
            /* The map only lives on the stack: overlays restore keys from the base colours meanwhile. */
            g_rgb_shown_layer_colors = NULL;
            rgb_paint_layer_colors(file_colors);
            g_rgb_force_render = 1;
            lightbar_request_redraw();
            return;
        }
    }

    /* Layer 0, or a layer without a colour map: back to the base colours. */
    if (g_rgb_shown_color_layer != 0) {
        g_rgb_shown_color_layer = 0;
        g_rgb_layer_switch_pending = 0;
        rgb_paint_base_colors(0);
        g_rgb_force_render = 1;
    }
}

/* True once the idle-off fade (overlay RGB_OVERLAY_ID_IDLE_FADE) has reached black or is gone. */
static bool rgb_idle_off_fade_done(void)
{
    for (uint8_t i = 0; i < g_rgb_overlay_count; ++i) {
        const rgb_overlay_slot_t *slot = &g_rgb_overlay_slots[i];

        if (slot->id != RGB_OVERLAY_ID_IDLE_FADE) {
            continue;
        }
        switch (slot->params.type) {
        case RGB_OVERLAY_FADE:
            return slot->params.u.fade.fade_ms <= slot->params.u.fade.elapsed_ms;
        case RGB_OVERLAY_LIGHTBAR_COLORS:
        case RGB_OVERLAY_SOLID_KEY:
            return true;
        default:
            return false;
        }
    }
    return true;
}

/*
 * Idle dimming: counts the time since the last key activity (rgb_reset_idle_timer) and starts
 * the dim fade and the off fade. Returns true when the LEDs are off, i.e. nothing is rendered.
 */
static bool rgb_idle_update(void)
{
    const IdleSettings *idle = &g_profile_global_settings.idle;
    rgb_overlay_params_t params;
    uint32_t previous_ms;
    uint32_t now_ms;
    uint32_t minutes;
    uint32_t off_minutes;

    if (!g_profile_global_settings.has_idle) {
        g_rgb_frame_stamp_ms = g_clock_uptime_ms;
        return false;
    }

    previous_ms = g_rgb_frame_stamp_ms;
    now_ms = g_clock_uptime_ms;
    g_rgb_frame_stamp_ms = now_ms;
    g_rgb_idle_timer.ms = (uint16_t)(g_rgb_idle_timer.ms + (now_ms - previous_ms));

    if (g_rgb_idle_timer.ms >= RGB_IDLE_MS_PER_SECOND) {
        /* The remainder above one second is dropped; a minute takes 61 counted seconds. */
        g_rgb_idle_timer.seconds++;
        g_rgb_idle_timer.ms = 0;
        if (g_rgb_idle_timer.seconds > RGB_IDLE_SECONDS_MAX) {
            g_rgb_idle_timer.seconds = 0;
            if (g_rgb_idle_timer.minutes <= RGB_IDLE_MINUTES_MAX) {
                g_rgb_idle_timer.minutes++;
            }
        }
    }
    minutes = g_rgb_idle_timer.minutes;
    off_minutes = idle->off_after_minutes;

    if (minutes >= off_minutes && (!idle->has_off_enabled || idle->off_enabled)) {
        if ((g_rgb_idle_flags & RGB_IDLE_FLAG_OFF) == 0) {
            /* Start from the dimmed level unless dim and off are due at the same minute. */
            rgb_fade_params(&params,
                            idle->dim_after_minutes == off_minutes ? RGB_FADE_LEVEL_FULL : RGB_FADE_LEVEL_DIMMED, 0,
                            RGB_IDLE_OFF_FADE_MS);
            if (g_rgb_sdk_mode_active == 0) {
                rgb_overlay_start(RGB_OVERLAY_ID_IDLE_FADE, RGB_OVERLAY_DURATION_FOREVER, &params);
            }
            g_rgb_idle_flags |= RGB_IDLE_FLAG_OFF;
        }
        if (rgb_idle_off_fade_done()) {
            g_rgb_effect_stamp_ms = g_clock_uptime_ms;
            return true;
        }
    } else if (idle->dim_after_minutes <= minutes && (g_rgb_idle_flags & RGB_IDLE_FLAG_DIMMED) == 0) {
        rgb_fade_params(&params, RGB_FADE_LEVEL_FULL, RGB_FADE_LEVEL_DIMMED, RGB_IDLE_DIM_FADE_MS);
        if (g_rgb_sdk_mode_active == 0) {
            rgb_overlay_start(RGB_OVERLAY_ID_IDLE_FADE, RGB_OVERLAY_DURATION_FOREVER, &params);
        }
        g_rgb_idle_flags |= RGB_IDLE_FLAG_DIMMED;
    }
    return false;
}

/* Takes the first entry out of g_rgb_dirty_pages; negative when the queue is empty. */
static int32_t rgb_dirty_page_pop(void)
{
    uint32_t irq_state = hal_irq_save();
    int32_t page;

    page = g_rgb_dirty_pages[0];
    if (page >= 0) {
        for (int i = 0; i != RGB_DIRTY_PAGE_QUEUE_LEN - 1; ++i) {
            g_rgb_dirty_pages[i] = g_rgb_dirty_pages[i + 1];
            g_rgb_dirty_pages[i + 1] = RGB_DIRTY_PAGE_FREE;
        }
        g_rgb_dirty_pages[RGB_DIRTY_PAGE_QUEUE_LEN - 1] = RGB_DIRTY_PAGE_FREE;
    }
    hal_irq_restore(irq_state);
    return page;
}

/*
 * Flush step: sends one queued page of the main frame to the LED driver. Key LEDs use the
 * profile brightness, the light-bar channels of the page the light-bar brightness; both are
 * multiplied by the fade level.
 */
static void rgb_send_dirty_page(void)
{
    int32_t queued = rgb_dirty_page_pop();
    const rgb_frame_page_t *page;
    rgb_frame_page_t out;
    uint16_t fade_level;
    uint16_t key_scale;
    uint32_t length;

    if (queued < 0 || queued > RGB_FRAME_PAGE_COUNT - 1) {
        return;
    }
    page = &g_rgb_frame_main.pages[(uint8_t)queued];

    memset(&out, 0, sizeof(out));
    fade_level = g_rgb_fade_level;
    key_scale = k_rgb_gamma_table[g_rgb_applied_brightness * (uint32_t)g_rgb_fade_level / 255];

    length = page->length;
    if (length >= RGB_FRAME_PAGE_PWM_MAX) {
        length = RGB_FRAME_PAGE_PWM_MAX;
    }
    for (uint32_t i = 0; i < length; ++i) {
        out.pwm[i] = rgb_output_value(page->pwm[i], key_scale);
    }

    lightbar_apply_page_brightness(page, &out, fade_level);

    led_matrix_write(page->driver_page, page->first_reg, out.pwm, length);
}

/*
 * Effect layers are rendered only at scan mode level 0, with at least one layer, while the
 * base colours are shown and the host does not drive the LEDs. Returns whether they are
 * rendered (*enabled) and whether one of them needs a new frame (return value).
 */
static uint32_t rgb_effects_need_render(bool *enabled)
{
    const rgb_effect_layer_t *layer;
    const rgb_effect_layer_t *end;
    uint32_t dirty;

    *enabled = false;
    if ((g_run_state_flags >> RUN_STATE_SCAN_MODE_SHIFT) != 0 || g_rgb_effect_layer_count == 0) {
        return 0;
    }
    if (g_rgb_shown_color_layer != 0 || g_rgb_sdk_mode_active != 0) {
        return 0;
    }

    *enabled = true;
    if (g_rgb_effect_redraw_request != 0) {
        return g_rgb_effect_redraw_request;
    }
    if (g_rgb_effect_reset_request != 0) {
        return g_rgb_effect_reset_request;
    }

    /* Event-driven kinds have a dirty / active flag; every other kind animates all the time. */
    layer = g_rgb_effect_layers;
    end = &g_rgb_effect_layers[(uint8_t)(g_rgb_effect_layer_count - 1) + 1];
    do {
        switch (layer->cfg.which_effect) {
        case RgbEffectLayer_key_ripple_tag:
        case RgbEffectLayer_reactive_fade_tag:
            dirty = layer->state.ripple.active; /* same offset as reactive.active */
            break;
        case RgbEffectLayer_level_meter_tag:
            dirty = layer->state.level_meter.dirty;
            break;
        case RgbEffectLayer_analog_spotlight_tag:
        case RgbEffectLayer_analog_glow_tag:
        case RgbEffectLayer_key_test_tag:
            dirty = layer->state.key_map.dirty;
            break;
        case RgbEffectLayer_analog_rgb_fill_tag:
            dirty = layer->state.fill.dirty;
            break;
        default:
            return 1;
        }
        ++layer;
    } while (dirty == 0 && layer != end);
    return dirty;
}

/* True when an overlay has to be drawn: start requested, slot stopped, blink toggle due or fade running. */
static bool rgb_overlays_need_render(uint16_t since_render_ms)
{
    if (g_rgb_overlay_dirty_mask != 0) {
        return true;
    }

    for (uint8_t i = 0; i < g_rgb_overlay_count; ++i) {
        const rgb_overlay_slot_t *slot = &g_rgb_overlay_slots[i];

        if (slot->redraw_class == 0) {
            continue;
        }
        if (slot->duration_ms == 0) {
            return true;
        }
        if (slot->params.type == RGB_OVERLAY_BLINK_KEY) {
            int32_t phase = slot->params.u.blink_key.phase_ms;
            int32_t period = slot->params.u.blink_key.period_ms;
            int32_t half_period = period >> 1;

            /* The phase wraps, or passes the middle of the period, within this frame. */
            if (since_render_ms + phase > period || (phase <= half_period && since_render_ms + phase > half_period)) {
                return true;
            }
        } else if (slot->params.type == RGB_OVERLAY_FADE
                   && slot->params.u.fade.fade_ms > slot->params.u.fade.elapsed_ms) {
            return true;
        }
    }
    return false;
}

/* Copies the PWM bytes of a rendered scratch frame into the main frame and queues the pages that changed. */
static void rgb_frame_merge(const rgb_frame_t *rendered)
{
    for (int index = 0; index != RGB_FRAME_PAGE_COUNT; ++index) {
        rgb_frame_page_t *page = &g_rgb_frame_main.pages[index];
        const rgb_frame_page_t *source = &rendered->pages[index];
        bool changed = false;

        /* The page headers of a scratch frame are zero; lengths come from the main frame. */
        for (uint32_t i = 0; i < page->length; ++i) {
            if (page->pwm[i] != source->pwm[i]) {
                page->pwm[i] = source->pwm[i];
                changed = true;
            }
        }
        if (changed) {
            rgb_mark_page_dirty((uint8_t)index);
        }
    }
}

/*
 * Draws the overlays and the light bar: the overlays of the light bar's class (fades, host
 * light-bar colours) are drawn by the light bar's render pass on top of it, the others below
 * it; without a light bar every overlay is drawn here in one pass.
 */
static void rgb_render_overlays_and_lightbar(rgb_frame_t *frame, const uint16_t *layer_colors, uint16_t elapsed_ms)
{
    if (g_rgb_sdk_mode_active == 0) {
        rgb_render_overlays(frame, layer_colors, RGB_OVERLAY_CLASS_ALL & (uint8_t)~lightbar_overlay_class(),
                            (uint16_t)g_rgb_effect_stamp_ms, elapsed_ms);
    }
    lightbar_render_pass(frame, layer_colors, elapsed_ms);
}

/*
 * RGB task step, called on every main-loop pass while the LED driver is enabled. Applies a
 * pending layer colour switch and the idle timer, then either renders a frame (effect layers,
 * overlays, light bar; at most every 16 ms and only when something changed) or sends one
 * queued page of the main frame to the LED driver.
 */
void rgb_update_frame(void)
{
    rgb_frame_t scratch;
    const uint16_t *layer_colors;
    uint32_t force;
    uint32_t stamp_ms;
    uint16_t since_render_ms;
    uint16_t elapsed_ms;
    bool effects_enabled;
    uint32_t effects_dirty;
    bool lightbar_dirty;

    rgb_apply_layer_colors();

    if (rgb_idle_update()) {
        rgb_send_dirty_page();
        return;
    }

    /* Frame period: 16 ms since the last render pass unless a render is forced. */
    force = g_rgb_force_render;
    if (force != 0) {
        stamp_ms = g_clock_uptime_ms;
        g_rgb_effect_stamp_ms = stamp_ms;
        since_render_ms = (uint16_t)(g_clock_uptime_ms - stamp_ms);
    } else {
        uint32_t since_ms;

        stamp_ms = g_rgb_effect_stamp_ms;
        since_ms = g_clock_uptime_ms - stamp_ms;
        if (since_ms <= RGB_FRAME_PERIOD_MS - 1) {
            rgb_send_dirty_page();
            return;
        }
        since_render_ms = (uint16_t)since_ms;
    }

    layer_colors = (g_rgb_shown_color_layer != 0) ? g_rgb_shown_layer_colors : NULL;

    /* Does anything need a new frame? */
    effects_dirty = rgb_effects_need_render(&effects_enabled);
    lightbar_dirty = lightbar_needs_render(since_render_ms);
    if (!rgb_overlays_need_render(since_render_ms) && effects_dirty == 0 && !lightbar_dirty && force == 0) {
        rgb_send_dirty_page();
        return;
    }

    g_rgb_effect_stamp_ms = g_clock_uptime_ms;
    elapsed_ms = (uint16_t)(g_rgb_effect_stamp_ms - stamp_ms);
    g_rgb_force_render = 0;

    if (effects_enabled) {
        /* Effects repaint every key: compose in a scratch frame and send only what changed. */
        memset(&scratch, 0, sizeof(scratch));
        rgb_render_effect_layers(&scratch, elapsed_ms);
        rgb_render_overlays_and_lightbar(&scratch, layer_colors, elapsed_ms);
        rgb_frame_merge(&scratch);
    } else {
        /* Static colours: overlays and the light bar are drawn straight into the main frame. */
        rgb_render_overlays_and_lightbar(&g_rgb_frame_main, layer_colors, elapsed_ms);
    }
}
