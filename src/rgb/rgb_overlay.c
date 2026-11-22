/*
 * rgb_overlay.c - timed overlays drawn on top of the frame: single keys (solid / blinking),
 * the global brightness fade and the light-bar colours of feature lightbar.
 *
 * Slots live in g_rgb_overlay_slots (g_rgb_overlay_count used). rgb_overlay_start and
 * rgb_overlay_stop only edit the table; rgb_render_overlays advances and draws the slots from
 * rgb_update_frame. All three run with interrupts masked.
 */
#include "rgb/rgb_frame.h"

#include <string.h>

#include "core/clock.h"
#include "hal/hal_irq.h"
#include "rgb/lightbar/lightbar.h"

/* Redraw class of an overlay type. */
static uint8_t rgb_overlay_class_of_type(uint8_t type)
{
    switch (type) {
    case RGB_OVERLAY_SOLID_KEY:
    case RGB_OVERLAY_BLINK_KEY:
        return RGB_OVERLAY_CLASS_KEYS;
    case RGB_OVERLAY_FADE:
        return RGB_OVERLAY_CLASS_GLOBAL;
    case RGB_OVERLAY_LIGHTBAR_COLORS:
        return lightbar_overlay_class();
    default:
        return 0;
    }
}

/* Ends the overlay with this id: its slot is marked finished and removed by the next render pass. */
void rgb_overlay_stop(uint32_t overlay_id)
{
    uint32_t irq_state;

    if (overlay_id == RGB_OVERLAY_ID_NONE) {
        return;
    }

    irq_state = hal_irq_save();
    for (uint8_t i = 0; i < g_rgb_overlay_count; ++i) {
        if (g_rgb_overlay_slots[i].id == overlay_id) {
            /* Duration 0 = finished: the next render pass restores what the overlay covered and frees the slot. */
            g_rgb_overlay_slots[i].duration_ms = 0;
            break;
        }
    }
    hal_irq_restore(irq_state);
}

/*
 * Starts (or restarts) the overlay with this id for `duration` ms (RGB_OVERLAY_DURATION_FOREVER
 * = until stopped) with a copy of *params. The request is dropped when all five slots are used.
 */
void rgb_overlay_start(uint32_t overlay_id, uint16_t duration, const rgb_overlay_params_t *params)
{
    uint8_t count;
    uint8_t index;
    rgb_overlay_slot_t *slot;
    uint8_t redraw_class;
    uint32_t irq_state = hal_irq_save();

    count = g_rgb_overlay_count;

    for (index = 0; index < count; ++index) {
        if (g_rgb_overlay_slots[index].id == overlay_id) {
            break;
        }
    }

    if (index < count) {
        /* Restart of a running overlay: its old class must be redrawn as well (the type may change). */
        slot = &g_rgb_overlay_slots[index];
        g_rgb_overlay_dirty_mask |= slot->redraw_class;
    } else {
        if (count == RGB_OVERLAY_SLOT_COUNT) {
            /* Table full: the request is dropped. */
            hal_irq_restore(irq_state);
            return;
        }
        g_rgb_overlay_count = count + 1;
        slot = &g_rgb_overlay_slots[count];
    }

    slot->id = (uint8_t)overlay_id;
    slot->duration_ms = duration;
    slot->restart = 1;
    slot->start_tick = g_clock_uptime_ms;
    memcpy(&slot->params, params, sizeof(slot->params));

    redraw_class = rgb_overlay_class_of_type(params->type);
    slot->redraw_class = redraw_class;
    g_rgb_overlay_dirty_mask |= redraw_class;

    hal_irq_restore(irq_state);
}

/*
 * Sets the global brightness factor of a fade overlay; a change re-sends every page unless
 * both brightness values are 0.
 */
static void rgb_overlay_set_fade_level(uint8_t level)
{
    if (g_rgb_fade_level != level) {
        g_rgb_fade_level = level;
        if ((lightbar_brightness() | g_rgb_applied_brightness) != 0) {
            for (int page = 0; page != RGB_FRAME_PAGE_COUNT; ++page) {
                rgb_mark_page_dirty((uint8_t)page);
            }
        }
    }
}

/*
 * Gives a key back its normal colour when its overlay ends. Only done in the main frame: a
 * scratch frame is rebuilt from scratch anyway.
 */
static void rgb_overlay_restore_key(rgb_frame_t *frame, const uint16_t *layer_colors, uint8_t key_pos)
{
    uint32_t row = RGB_KEY_POS_ROW(key_pos);
    uint32_t col = RGB_KEY_POS_COL(key_pos);
    uint16_t rgb565;
    uint32_t argb;

    if (frame != &g_rgb_frame_main) {
        return;
    }
    if (row > RGB_MATRIX_ROWS - 1 || col > RGB_MATRIX_COLS - 1) {
        return;
    }

    if (layer_colors != NULL) {
        rgb565 = layer_colors[RGB_MATRIX_COLS * row + col];
    } else {
        rgb565 = g_rgb_base_colors[row][col];
    }
    argb = rgb_convert_rgb565_to_argb(rgb565);
    rgb_frame_set_led(&g_rgb_frame_main, g_rgb_key_to_led[RGB_MATRIX_COLS * row + col], (uint8_t)(argb >> 16),
                      (uint8_t)(argb >> 8), (uint8_t)argb);
}

/* LED id of an overlay key. The position is not range checked while the overlay runs. */
static uint32_t rgb_overlay_key_led(uint8_t key_pos)
{
    return g_rgb_key_to_led[RGB_MATRIX_COLS * RGB_KEY_POS_ROW(key_pos) + RGB_KEY_POS_COL(key_pos)];
}

/* Undoes the visible effect of a finished slot. */
static void rgb_overlay_finish(rgb_frame_t *frame, const uint16_t *layer_colors, const rgb_overlay_slot_t *slot)
{
    switch (slot->params.type) {
    case RGB_OVERLAY_SOLID_KEY:
        rgb_overlay_restore_key(frame, layer_colors, slot->params.u.solid_key.key_pos);
        break;
    case RGB_OVERLAY_BLINK_KEY:
        rgb_overlay_restore_key(frame, layer_colors, slot->params.u.blink_key.key_pos);
        break;
    case RGB_OVERLAY_FADE:
        rgb_overlay_set_fade_level(RGB_FADE_LEVEL_FULL);
        break;
    case RGB_OVERLAY_LIGHTBAR_COLORS:
        lightbar_clear_overlay(frame);
        break;
    default:
        break;
    }
}

/* Advances a running slot by elapsed ms and draws it. */
static void rgb_overlay_draw(rgb_frame_t *frame, rgb_overlay_slot_t *slot, uint16_t elapsed)
{
    switch (slot->params.type) {
    case RGB_OVERLAY_SOLID_KEY: {
        const rgb_overlay_solid_key_t *key = &slot->params.u.solid_key;

        rgb_frame_set_led(frame, rgb_overlay_key_led(key->key_pos), key->color.r, key->color.g, key->color.b);
        break;
    }
    case RGB_OVERLAY_BLINK_KEY: {
        rgb_overlay_blink_key_t *blink = &slot->params.u.blink_key;
        uint32_t period = blink->period_ms;
        uint16_t phase = elapsed;
        const rgb_argb_t *color;

        /* KNOWN ISSUE: a period of 0 never leaves this loop (interrupts are masked) once time has passed.
         * Matches stock firmware 2.14.1. */
        while (phase > period) {
            phase = (uint16_t)(phase - period);
        }
        phase = (uint16_t)(phase + blink->phase_ms);
        if (phase >= period) {
            phase = (uint16_t)(phase - period);
        }
        blink->phase_ms = phase;

        color = (phase > (period >> 1)) ? &blink->color_b : &blink->color_a;
        rgb_frame_set_led(frame, rgb_overlay_key_led(blink->key_pos), color->r, color->g, color->b);
        break;
    }
    case RGB_OVERLAY_FADE: {
        rgb_overlay_fade_t *fade = &slot->params.u.fade;
        int32_t fade_ms = fade->fade_ms;
        int32_t total = fade->elapsed_ms + elapsed;

        if (total > fade_ms) {
            rgb_overlay_set_fade_level(fade->to);
            if (fade->elapsed_ms < fade->fade_ms) {
                fade->elapsed_ms = fade->fade_ms;
            }
        } else {
            uint8_t progress;

            fade->elapsed_ms = (uint16_t)total;
            /* KNOWN ISSUE: fade_ms == 0 with no elapsed time divides by zero (the Cortex-M4 UDIV yields 0).
             * Matches stock firmware 2.14.1. */
            progress = (uint8_t)(255u * (uint32_t)(uint16_t)total / (uint32_t)fade_ms);
            rgb_overlay_set_fade_level((uint8_t)((uint16_t)((fade->from << 8) + progress * (fade->to - fade->from)) >> 8));
        }
        break;
    }
    case RGB_OVERLAY_LIGHTBAR_COLORS:
        lightbar_draw_overlay(frame, &slot->params.u.lightbar);
        break;
    default:
        break;
    }
}

/*
 * Advances and draws the overlay slots whose redraw class is in class_mask into frame, and
 * removes finished slots (restoring the key colour from layer_colors or the base colours).
 * now_ms and elapsed_ms are the time of this render pass and the time since the previous one.
 */
void rgb_render_overlays(rgb_frame_t *frame, const uint16_t *layer_colors, uint8_t class_mask, uint32_t now_ms,
                         uint16_t elapsed_ms)
{
    uint8_t index = 0;
    uint32_t irq_state = hal_irq_save();

    while (index < g_rgb_overlay_count) {
        rgb_overlay_slot_t *slot = &g_rgb_overlay_slots[index];
        uint16_t elapsed;

        if ((class_mask & slot->redraw_class) == 0) {
            ++index;
            continue;
        }

        if (slot->duration_ms == 0) {
            uint8_t last = (uint8_t)(g_rgb_overlay_count - 1);

            /* Finished: undo it and close the gap with the last slot, which is then examined in this place. */
            rgb_overlay_finish(frame, layer_colors, slot);
            if (last == index) {
                ++index;
            } else {
                *slot = g_rgb_overlay_slots[last];
            }
            g_rgb_overlay_count = last;
            continue;
        }

        if (slot->restart != 0) {
            /* First pass after rgb_overlay_start: the time since the start counts. */
            elapsed = (uint16_t)((uint16_t)now_ms - slot->start_tick);
            slot->restart = 0;
        } else {
            elapsed = elapsed_ms;
        }

        rgb_overlay_draw(frame, slot, elapsed);

        if (slot->duration_ms != RGB_OVERLAY_DURATION_FOREVER) {
            if (elapsed <= slot->duration_ms) {
                slot->duration_ms = (uint16_t)(slot->duration_ms - elapsed);
            } else {
                slot->duration_ms = 0;
            }
        }
        ++index;
    }

    g_rgb_overlay_dirty_mask &= (uint8_t)~class_mask;
    hal_irq_restore(irq_state);
}
