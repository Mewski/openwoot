/*
 * lightbar_effects.c - renderer of one light-bar entry: evaluates the condition tree
 * and paints the effect (meter, profile indicator, mirror, blink, static colour, custom
 * colours) into an LED frame.
 *
 * Light-bar positions 0..9 map to LED ids through k_lightbar_led_ids, key rectangles
 * through g_rgb_key_to_led (17 per row). The static functions below are the steps of
 * rgb_render_lightbar_effect.
 */
#include <string.h>

#include "rgb/lightbar/lightbar_internal.h"
#include "rgb/rgb_frame.h"

/* Bytes of a colour word 0xAARRGGBB as returned by rgb_unpack_color / rgb_blend_argb. */
#define ARGB_B(c) ((uint8_t)(c))
#define ARGB_G(c) ((uint8_t)((c) >> 8))
#define ARGB_R(c) ((uint8_t)((c) >> 16))
#define ARGB_A(c) ((uint8_t)((c) >> 24))
#define ARGB(a, r, g, b) \
    (((uint32_t)(a) << 24) | ((uint32_t)(r) << 16) | ((uint32_t)(g) << 8) | (uint32_t)(b))

#define LIGHTBAR_POS_MAX (LIGHTBAR_LED_COUNT - 1)

/*
 * Paints an inclusive range of light-bar positions; positions above 9 are skipped.
 * KNOWN ISSUE: with last == 255 the 8-bit position wraps and the loop never ends. Matches
 * stock firmware 2.14.1.
 */
static void lightbar_fill_range(rgb_frame_t *frame, const LightbarRange *range, uint8_t r, uint8_t g, uint8_t b)
{
    for (uint8_t pos = range->first; pos <= range->last; pos++) {
        if (pos <= LIGHTBAR_POS_MAX) {
            rgb_frame_set_led(frame, k_lightbar_led_ids[pos], r, g, b);
        }
    }
}

/*
 * Paints an inclusive rectangle of keys. Rows and columns are not range checked.
 * KNOWN ISSUE: a row or column of 255 never ends, values past the 6 x 17 matrix read
 * beyond the key -> LED table. Matches stock firmware 2.14.1.
 */
static void lightbar_fill_key_rect(rgb_frame_t *frame, const KeyRect *rect, uint8_t r, uint8_t g, uint8_t b)
{
    for (uint8_t row = rect->first.row; row <= rect->last.row; row++) {
        for (uint8_t col = rect->first.col; col <= rect->last.col; col++) {
            rgb_frame_set_led(frame, g_rgb_key_to_led[RGB_MATRIX_COLS * row + col], r, g, b);
        }
    }
}

/*
 * Evaluates the condition tree of an entry for this frame and records the result.
 * *changed receives result ^ previous result. Returns false when the entry is not to be
 * drawn.
 */
static bool lightbar_update_condition(const LightbarEntry *entry, rgb_lightbar_state_t *state,
                                      uint32_t elapsed_ms, uint8_t *changed)
{
    const LightbarCondition *condition = &entry->condition;
    uint8_t previous;
    int result;

    switch (condition->op) {
    case LightbarLogicOp_LIGHTBAR_LOGIC_OP_AND:
        result = 1; /* an AND without groups is true */
        for (uint32_t group = 0; group < condition->groups_count; group++) {
            result = (uint8_t)(rgb_lightbar_eval_condition_group(&condition->groups[group],
                                                                 state->cond.groups[group],
                                                                 elapsed_ms) & result);
        }
        break;

    case LightbarLogicOp_LIGHTBAR_LOGIC_OP_OR:
        if (condition->groups_count == 0) {
            state->cond.result = 0;
            state->cond.dirty = 0;
            return false;
        }
        result = 0;
        for (uint32_t group = 0; group < condition->groups_count; group++) {
            result = (uint8_t)(rgb_lightbar_eval_condition_group(&condition->groups[group],
                                                                 state->cond.groups[group],
                                                                 elapsed_ms) | result);
        }
        break;

    case LightbarLogicOp_LIGHTBAR_LOGIC_OP_SINGLE:
        if (condition->groups_count != 1) {
            state->cond.result = 0;
            state->cond.dirty = 0;
            return false;
        }
        result = rgb_lightbar_eval_condition_group(&condition->groups[0], state->cond.groups[0], elapsed_ms);
        break;

    default:
        state->cond.result = 0;
        state->cond.dirty = 0;
        return false;
    }

    state->cond.dirty = 0;
    previous = state->cond.result;
    state->cond.result = (uint8_t)result;
    *changed = (uint8_t)(result ^ previous);
    return result != 0;
}

/*
 * Meter: lights value / max_value of a light-bar range, the boundary LED partially. Lit
 * LEDs blend from the background (configured, or what is in the frame) to the meter colour
 * or to a gradient between gradient_start_color and color.
 */
static void lightbar_paint_meter(rgb_frame_t *frame, const LightbarMeter *meter, rgb_lightbar_meter_state_t *meter_state)
{
    bool reverse;
    uint32_t lit_color;
    uint32_t start_color;
    uint8_t bg_b = 0;
    uint8_t bg_g = 0;
    uint8_t bg_r = 0;
    uint8_t bg_a = 0xFF;
    uint8_t led_count;
    float lit_length;

    /* a key rectangle target (tag 6) is in the schema but never drawn */
    if (meter->which_target != LightbarMeter_range_tag) {
        return;
    }
    /* a host signal is shown only once the host has touched the host-signal table */
    if (!meter->signal.is_builtin && !signals_host_values_active()) {
        return;
    }

    reverse = meter->has_reverse ? meter->reverse : false;

    lit_color = rgb_unpack_color(meter->color);
    if (meter->has_gradient_start_color) {
        start_color = rgb_unpack_color(meter->gradient_start_color);
    } else {
        start_color = lit_color;
    }
    if (meter->has_background_color) {
        uint32_t background = rgb_unpack_color(meter->background_color);

        bg_b = ARGB_B(background);
        bg_g = ARGB_G(background);
        bg_r = ARGB_R(background);
        bg_a = ARGB_A(background);
    }

    led_count = (uint8_t)(meter->target.range.last + 1 - meter->target.range.first);
    lit_length = (meter_state->value / meter->max_value) * (float)(int32_t)led_count;

    for (uint8_t i = 0; i < led_count; i++) {
        uint8_t pos;
        uint8_t led_id;
        float led_index = (float)(int32_t)i;
        uint8_t level;

        if (reverse) {
            pos = (uint8_t)(meter->target.range.last - i);
        } else {
            pos = (uint8_t)(meter->target.range.first + i);
        }
        if (pos > LIGHTBAR_POS_MAX) {
            continue;
        }
        led_id = k_lightbar_led_ids[pos];

        if ((uint8_t)(uint32_t)lit_length == i) {
            /* the boundary LED shows the fraction */
            level = (uint8_t)(uint32_t)((lit_length - led_index) * 255.0f);
        } else {
            level = (lit_length > led_index) ? 0xFF : 0;
        }

        if (lit_length > led_index) {
            uint32_t color;
            uint32_t blended;

            if (!meter->has_background_color) {
                uint8_t page_r, page_g, page_b;
                uint8_t *ch_r, *ch_g, *ch_b;

                /* No background colour: blend from what is in the frame. The colour read
                 * here is kept for the following LEDs when a later lookup fails. */
                if (rgb_frame_locate_led(frame, led_id, &page_r, &page_g, &page_b, &ch_r, &ch_g, &ch_b) != 0) {
                    bg_r = *ch_r;
                    bg_g = *ch_g;
                    bg_b = *ch_b;
                }
            }
            color = lit_color;
            if (meter->has_gradient_start_color) {
                color = rgb_blend_argb(start_color, lit_color, (uint8_t)(255u * i / led_count));
            }
            blended = rgb_blend_argb(ARGB(bg_a, bg_r, bg_g, bg_b), color, level);
            rgb_frame_set_led(frame, led_id, ARGB_R(blended), ARGB_G(blended), ARGB_B(blended));
        } else if (meter->has_background_color) {
            rgb_frame_set_led(frame, led_id, bg_r, bg_g, bg_b);
        }
    }

    meter_state->dirty = 0;
}

/*
 * Profile indicator: optional background over the whole range, then the segment of the
 * active profile in the profile colour. Signal RGB_SIGNAL_ACTIVE_PROFILE carries
 * (index + 1) | namespace << 8; namespace 1 lights the last (up to) two LEDs instead.
 * KNOWN ISSUE: unlike the other painters the positions are not checked against 9, so a
 * range past the light bar reads beyond k_lightbar_led_ids (and a range ending at 255
 * never ends). Matches stock firmware 2.14.1.
 */
static void lightbar_paint_profile_indicator(rgb_frame_t *frame, const LightbarProfileIndicator *indicator,
                                             rgb_lightbar_profile_state_t *profile_state)
{
    uint32_t profile_value;
    uint32_t segment_index;
    uint32_t profile_namespace;
    uint8_t color_b, color_g, color_r;
    uint8_t first;
    uint8_t last;
    uint8_t led_count;
    uint8_t segment_length;
    uint8_t segment_start;
    int32_t segment_end;

    /* a key rectangle target (tag 2) is in the schema but never drawn */
    if (indicator->which_target != LightbarProfileIndicator_range_tag) {
        return;
    }
    first = indicator->target.range.first;
    if (first == 0 && indicator->target.range.last == 0) {
        return;
    }

    profile_value = (uint32_t)profile_state->profile;
    if ((uint8_t)profile_value != 0) {
        segment_index = (uint8_t)((uint8_t)profile_value - 1);
        if (segment_index >= 4) {
            segment_index = 4;
        }
        profile_namespace = (uint16_t)profile_value >> 8;
    } else {
        segment_index = 0;
        profile_namespace = 0;
    }

    color_b = profile_state->profile_color.b;
    color_g = profile_state->profile_color.g;
    color_r = profile_state->profile_color.r;

    last = indicator->target.range.last;
    if (profile_state->has_background != 0 && first <= last) {
        uint8_t bg_b = profile_state->background.b;
        uint8_t bg_g = profile_state->background.g;
        uint8_t bg_r = profile_state->background.r;

        for (uint8_t pos = first; pos <= last; pos++) {
            rgb_frame_set_led(frame, k_lightbar_led_ids[pos], bg_r, bg_g, bg_b);
        }
    }

    led_count = (uint8_t)((uint8_t)(last + 1) - first);
    if (profile_namespace == 1) {
        segment_length = (led_count >= 2) ? 2 : led_count;
        segment_start = (uint8_t)((uint8_t)(last + 1) - segment_length);
    } else {
        uint32_t segment_count = indicator->has_segment_count ? indicator->segment_count : 1;
        /* KNOWN ISSUE: segment_count == 0 is not guarded (UDIV by zero gives 0 on the target).
         * Matches stock firmware 2.14.1. */
        float by_segments = (float)(int32_t)(led_count / segment_count);
        float by_profiles = (float)(int32_t)led_count / profile_state->profile_count;

        if (by_profiles < by_segments) {
            segment_length = (uint8_t)(uint32_t)by_profiles;
        } else {
            segment_length = (uint8_t)(uint32_t)by_segments;
        }
        segment_start = (uint8_t)(first + segment_length * segment_index);
        if (segment_length < 1) {
            segment_length = 1;
        }
    }

    segment_end = segment_length + segment_start;
    for (uint8_t pos = segment_start; pos < segment_end; pos++) {
        rgb_frame_set_led(frame, k_lightbar_led_ids[pos], color_r, color_g, color_b);
    }

    profile_state->dirty = 0;
}

/*
 * Mirror: samples the frame colours of a run of keys and resamples them onto a light-bar
 * range with linear interpolation. The keys are always taken along row first.row starting
 * at first.col; a rectangle spanning several rows only changes the number of samples
 * (last.row + 1 - first.row).
 */
static void lightbar_paint_mirror(rgb_frame_t *frame, const LightbarMirror *mirror)
{
    uint8_t sample_count;
    uint8_t dest_count;
    uint8_t pos;
    float sample_span;
    float dest_span;

    if (mirror->source.first.row == mirror->source.last.row) {
        sample_count = (uint8_t)(mirror->source.last.col + 1 - mirror->source.first.col);
    } else {
        sample_count = (uint8_t)(mirror->source.last.row + 1 - mirror->source.first.row);
    }

    /*
     * The sample array has two spare words and is cleared: a key without LED gives a black
     * sample, and the interpolation reads the word after the last sample (with weight 0
     * for the last destination LED) as black. Stock firmware 2.14.1 uses an uninitialised
     * array of exactly sample_count words there, so its result for these cases is undefined.
     */
    uint32_t samples[sample_count + 2];

    memset(samples, 0, sizeof(samples));

    for (uint8_t i = 0; i < sample_count; i++) {
        uint8_t page_r, page_g, page_b;
        uint8_t *ch_r, *ch_g, *ch_b;
        uint8_t led_id = g_rgb_key_to_led[RGB_MATRIX_COLS * mirror->source.first.row
                                          + (uint8_t)(mirror->source.first.col + i)];

        if (rgb_frame_locate_led(frame, led_id, &page_r, &page_g, &page_b, &ch_r, &ch_g, &ch_b) != 0) {
            /* bytes 0..2 only; the alpha byte is left alone */
            samples[i] = (samples[i] & 0xFF000000u) | ARGB(0, *ch_r, *ch_g, *ch_b);
        }
    }

    pos = mirror->range.first;
    dest_count = (uint8_t)(mirror->range.last + 1 - pos);
    if (dest_count == 0 || pos > LIGHTBAR_POS_MAX) {
        return;
    }

    sample_span = (float)((int32_t)sample_count - 1);
    dest_span = (float)((int32_t)dest_count - 1); /* 0 for a single LED: 0 / 0, converted to sample 0 */

    for (uint8_t i = 0;;) {
        uint8_t led_id = k_lightbar_led_ids[pos];
        float sample_pos = (float)(uint32_t)i / dest_span * sample_span;
        uint8_t sample_index = (uint8_t)(uint32_t)sample_pos;
        float fraction = sample_pos - (float)sample_index;
        /* the only double arithmetic of the renderer (__aeabi_f2d, __aeabi_dmul, __aeabi_d2uiz) */
        uint8_t weight = (uint8_t)(uint32_t)((double)fraction * 255.0);
        uint32_t color = rgb_blend_argb(samples[sample_index], samples[(uint8_t)(sample_index + 1)], weight);

        rgb_frame_set_led(frame, led_id, ARGB_R(color), ARGB_G(color), ARGB_B(color));

        i++;
        if (i == dest_count) {
            break;
        }
        pos = (uint8_t)(mirror->range.first + i);
        if (pos > LIGHTBAR_POS_MAX) {
            break;
        }
    }
}

/*
 * Blink: periodic blend between color_a and color_b (black with alpha 0 when absent) on a
 * light-bar range or a key rectangle. The phase restarts when the condition result of the
 * entry changed in this frame.
 */
static void lightbar_paint_blink(rgb_frame_t *frame, const LightbarBlink *blink, rgb_lightbar_blink_state_t *blink_state,
                                 uint16_t elapsed_ms, uint8_t cond_changed)
{
    uint16_t phase;
    float cycle;
    uint8_t weight;
    uint32_t color_a;
    uint32_t color_b;
    uint32_t color;

    if (cond_changed != 0) {
        phase = 0;
    } else {
        /* KNOWN ISSUE: period_ms == 0 is not guarded (UDIV by zero gives 0 on the target,
         * the phase then keeps counting and cycle becomes NaN / infinite). Matches stock
         * firmware 2.14.1. */
        phase = (uint16_t)((uint16_t)(blink_state->phase_ms + elapsed_ms) % blink->period_ms);
    }
    blink_state->phase_ms = phase;
    cycle = (float)phase / (float)blink->period_ms; /* 0..1 */

    /* has_waveform is not tested: an absent waveform reads as 0 = square */
    switch (blink->waveform) {
    case LightbarBlinkWaveform_LIGHTBAR_BLINK_WAVEFORM_SQUARE:
        /* color_a in the first and last quarter, color_b in between */
        weight = (cycle >= 0.25f && cycle < 0.75f) ? 0xFF : 0;
        break;

    case LightbarBlinkWaveform_LIGHTBAR_BLINK_WAVEFORM_TRIANGLE:
        if (cycle > 0.5f) {
            cycle = 1.0f - cycle;
        }
        weight = (uint8_t)(uint32_t)((cycle + cycle) * 255.0f);
        break;

    default:
        weight = 0;
        break;
    }

    color_a = rgb_unpack_color(blink->color_a);
    color_b = 0;
    if (blink->has_color_b) {
        color_b = rgb_unpack_color(blink->color_b);
    }
    color = rgb_blend_argb(color_a, color_b, weight);

    if (blink->which_target == LightbarBlink_range_tag) {
        lightbar_fill_range(frame, &blink->target.range, ARGB_R(color), ARGB_G(color), ARGB_B(color));
    } else {
        lightbar_fill_key_rect(frame, &blink->target.rect, ARGB_R(color), ARGB_G(color), ARGB_B(color));
    }
}

/* Static colour on a light-bar range, or on a key rectangle for any other target tag. */
static void lightbar_paint_static(rgb_frame_t *frame, const LightbarStatic *static_color)
{
    uint32_t color = rgb_unpack_color(static_color->color);

    if (static_color->which_target == LightbarStatic_range_tag) {
        lightbar_fill_range(frame, &static_color->target.range, ARGB_R(color), ARGB_G(color), ARGB_B(color));
    } else {
        lightbar_fill_key_rect(frame, &static_color->target.rect, ARGB_R(color), ARGB_G(color), ARGB_B(color));
    }
}

/*
 * Custom colours: r, g, b of light-bar position i at colors.bytes[3 * i]. colors.size is
 * not looked at; positions without bytes show whatever the array holds (zero after decode).
 * KNOWN ISSUE: a range ending at 255 never ends. Matches stock firmware 2.14.1.
 */
static void lightbar_paint_custom(rgb_frame_t *frame, const LightbarCustom *custom)
{
    for (uint8_t pos = custom->range.first; pos <= custom->range.last; pos++) {
        if (pos <= LIGHTBAR_POS_MAX) {
            const uint8_t *rgb = &custom->colors.bytes[3 * pos];

            rgb_frame_set_led(frame, k_lightbar_led_ids[pos], rgb[0], rgb[1], rgb[2]);
        }
    }
}

/*
 * Evaluates the condition of one light-bar entry and, if it holds (or there is none),
 * paints its effect into the frame. Unbound entries are skipped. Only caller:
 * rgb_render_lightbar.
 */
void rgb_render_lightbar_effect(rgb_frame_t *frame, uint32_t elapsed_ms, const LightbarEntry *entry, rgb_lightbar_state_t *state)
{
    uint8_t cond_changed = 0;

    if (state->bound == 0) {
        return;
    }
    if (entry->has_condition && !lightbar_update_condition(entry, state, elapsed_ms, &cond_changed)) {
        return;
    }

    switch (entry->which_effect) {
    case LightbarEntry_meter_tag:
        lightbar_paint_meter(frame, &entry->effect.meter, &state->effect.meter);
        break;
    case LightbarEntry_profile_indicator_tag:
        lightbar_paint_profile_indicator(frame, &entry->effect.profile_indicator, &state->effect.profile);
        break;
    case LightbarEntry_mirror_tag:
        lightbar_paint_mirror(frame, &entry->effect.mirror);
        break;
    case LightbarEntry_blink_tag:
        lightbar_paint_blink(frame, &entry->effect.blink, &state->effect.blink, (uint16_t)elapsed_ms, cond_changed);
        break;
    case LightbarEntry_static_color_tag:
        lightbar_paint_static(frame, &entry->effect.static_color);
        break;
    case LightbarEntry_custom_tag:
        lightbar_paint_custom(frame, &entry->effect.custom);
        break;
    default:
        break;
    }
}
