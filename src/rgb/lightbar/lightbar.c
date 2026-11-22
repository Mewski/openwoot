/*
 * lightbar.c - the light bar: configuration loading (start-up step for `glb`, profile section
 * for `lb`), binding of the entries to their signals, delivery of signal changes into the entry
 * states, brightness, the per-frame render / redraw-poll entry points and the light-bar overlay.
 *
 * Configuration: file `lb` of the active profile (g_rgb_lightbar_config) and the root file
 * `glb` (g_rgb_global_lightbar_config), up to four entries each. An entry is an optional
 * condition tree (lightbar_condition.c) plus one effect (lightbar_effects.c).
 */
#include <string.h>

#include "pb_encode.h"

#include "rgb/lightbar/lightbar_internal.h"

#include "core/run_state.h"
#include "core/startup.h"
#include "profile/profile.h"
#include "rgb/rgb_frame.h"
#include "storage/storage.h"
#include "storage/storage_data.h"

enum {
    STARTUP_LIGHTBAR_GLOBAL_CONFIG = STARTUP_FRAME_PAGES + 1, /* before the first profile activation */
};

enum {
    PROFILE_SECTION_LIGHTBAR = PROFILE_SECTION_EFFECT_START + 1,
};

/* Contrast colours of LightbarProfileBackground CONTRAST_COLOR, 0xAARRGGBB (rgb_lightbar_resolve_profile_color). */
#define LIGHTBAR_CONTRAST_YELLOW   0xFFFFFF00u
#define LIGHTBAR_CONTRAST_RED      0xFFFF0000u
#define LIGHTBAR_CONTRAST_MAGENTA  0xFFFF00FFu
#define LIGHTBAR_CONTRAST_BLUE     0xFF0000FFu
#define LIGHTBAR_CONTRAST_CYAN     0xFF00FFFFu
#define LIGHTBAR_CONTRAST_GREEN    0xFF00FF00u
#define LIGHTBAR_CONTRAST_MINT     0xFF19DE3Du

/* 0xAARRGGBB word -> colour struct (byte 0 = blue). */
static inline rgb_argb_t lightbar_argb_from_u32(uint32_t argb)
{
    rgb_argb_t color;

    color.b = (uint8_t)argb;
    color.g = (uint8_t)(argb >> 8);
    color.r = (uint8_t)(argb >> 16);
    color.a = (uint8_t)(argb >> 24);
    return color;
}

/* Raw bits of a float signal value as a colour (signal RGB_SIGNAL_PROFILE_COLOR carries 0xFFRRGGBB). */
static inline rgb_argb_t lightbar_argb_from_float_bits(float value)
{
    rgb_argb_t color;

    memcpy(&color, &value, sizeof(color));
    return color;
}

/*
 * Boot-time loader of the root file `glb`. A missing or invalid file leaves no entries.
 * Needs about 840 bytes of stack.
 */
void rgb_load_global_lightbar_config(void)
{
    char path[8];
    GlobalLightbarConfig config;

    memset(&config, 0, sizeof(config));
    if (g_storage_mounted != 0) {
        strcpy(path, "glb");
        storage_read_pb_file(path, &GlobalLightbarConfig_msg, &config);
    }
    memcpy(&g_rgb_global_lightbar_config, &config, sizeof(g_rgb_global_lightbar_config));
}
STARTUP_STEP(STARTUP_LIGHTBAR_GLOBAL_CONFIG, rgb_load_global_lightbar_config);

void lightbar_write_global_file(const GlobalLightbarConfig *config)
{
    uint8_t encoded[GlobalLightbarConfig_size];
    pb_ostream_t stream = pb_ostream_from_buffer(encoded, sizeof(encoded));

    pb_encode(&stream, &GlobalLightbarConfig_msg, config); /* result ignored, matches stock firmware 2.14.1 */
    storage_write_file("glb", encoded, (uint16_t)stream.bytes_written);
}

void lightbar_config_changed(void)
{
    g_rgb_lightbar_redraw = 1;
    g_rgb_lightbar_active = 0;
    if (g_rgb_lightbar_config.enabled) {
        rgb_lightbar_rebind_signals(&g_rgb_settings);
    } else {
        rgb_lightbar_set_brightness(0);
    }
}

/* Profile section: file "lb" -> g_rgb_lightbar_config; without the file the light bar is on and
 * shows the global entries at the key brightness. */
static void lightbar_section_load(uint16_t profile_id, uint32_t default_index)
{
    if (profile_read_pb_file(profile_id, "lb", &LightbarConfig_msg, &g_rgb_lightbar_config) != STORAGE_OK) {
        g_rgb_lightbar_config.enabled = true;
        g_rgb_lightbar_config.use_global = true;
        g_rgb_lightbar_config.has_brightness = false;
        g_rgb_lightbar_config.entries_count = 0;
        g_rgb_lightbar_config.brightness = 255;
    }
    lightbar_config_changed();
}
PROFILE_SECTION(PROFILE_SECTION_LIGHTBAR, lightbar_section_load);

void lightbar_apply_scan_mode_change(void)
{
    lightbar_config_changed();
}

/*
 * Rebuilds every signal subscription of the light-bar entries: drops all subscribers and
 * all host signals, applies the light-bar brightness, binds the `glb` entries (if enabled)
 * and the `lb` entries, and tells the host that the host-signal list changed.
 * g_rgb_lightbar_active is only ORed here; the callers clear it first.
 */
void rgb_lightbar_rebind_signals(const RgbSettings *rgb_settings)
{
    uint32_t brightness;

    signals_clear_subscribers();

    if (g_rgb_lightbar_config.has_brightness) {
        brightness = (uint8_t)g_rgb_lightbar_config.brightness;
    } else {
        brightness = rgb_settings->base.brightness;
    }
    rgb_lightbar_set_brightness(brightness);

    if (g_rgb_lightbar_config.use_global) {
        for (uint8_t i = 0; i < g_rgb_global_lightbar_config.entries_count; i++) {
            g_rgb_lightbar_active |= rgb_lightbar_init_item_state(
                RGB_SUBSCRIBER_LIGHTBAR(i) | RGB_SUBSCRIBER_GLOBAL,
                &g_rgb_global_lightbar_config.entries[i],
                &g_rgb_global_lightbar_states[i]);
        }
    }
    for (uint8_t i = 0; i < g_rgb_lightbar_config.entries_count; i++) {
        g_rgb_lightbar_active |= rgb_lightbar_init_item_state(
            RGB_SUBSCRIBER_LIGHTBAR(i),
            &g_rgb_lightbar_config.entries[i],
            &g_rgb_lightbar_states[i]);
    }

    protocol_send_lightbar_changed_event();
}

/*
 * Filter applied while the scan mode level is non-zero: the condition may only use the
 * signals of RGB_SIGNAL_FAST_MODE_MASK, the blink effect is not available and a meter must
 * show one of those signals.
 */
static bool lightbar_item_allowed_in_fast_mode(const LightbarEntry *item)
{
    uint8_t effect_kind;

    if (item->has_condition) {
        const LightbarCondition *condition = &item->condition;

        if (condition->op == LightbarLogicOp_LIGHTBAR_LOGIC_OP_SINGLE) {
            if (condition->groups_count != 1
                || rgb_lightbar_group_allowed_in_fast_mode(&condition->groups[0]) == 0) {
                return false;
            }
        } else if (condition->op == LightbarLogicOp_LIGHTBAR_LOGIC_OP_AND
                   || condition->op == LightbarLogicOp_LIGHTBAR_LOGIC_OP_OR) {
            uint32_t groups_count = condition->groups_count;

            for (uint32_t group = 0; group < groups_count; group++) {
                if (rgb_lightbar_group_allowed_in_fast_mode(&condition->groups[group]) == 0) {
                    return false;
                }
            }
        } else {
            return false;
        }
    }

    effect_kind = (uint8_t)item->which_effect; /* low byte only */
    switch (effect_kind) {
    case LightbarEntry_profile_indicator_tag:
    case LightbarEntry_mirror_tag:
    case LightbarEntry_static_color_tag:
    case LightbarEntry_custom_tag:
        return true;

    case LightbarEntry_meter_tag: {
        uint32_t signal_id = (uint8_t)item->effect.meter.signal.id; /* low byte only */

        return item->effect.meter.signal.is_builtin
            && signal_id <= RGB_SIGNAL_COUNT - 1
            && ((1u << signal_id) & RGB_SIGNAL_FAST_MODE_MASK) != 0;
    }

    default: /* no effect, blink, unknown */
        return false;
    }
}

/*
 * Prepares the run-time state of one light-bar entry and subscribes it to the signals it
 * depends on. subscriber_id = RGB_SUBSCRIBER_LIGHTBAR(index), plus RGB_SUBSCRIBER_GLOBAL for
 * `glb` entries. Returns non-zero when the entry is bound (the value is always 1).
 */
uint8_t rgb_lightbar_init_item_state(uint32_t subscriber_id, const LightbarEntry *item, rgb_lightbar_state_t *state)
{
    uint32_t effect_subscriber = subscriber_id | RGB_SUBSCRIBER_CLASS_EFFECT;
    uint32_t cond_subscriber = subscriber_id | RGB_SUBSCRIBER_CLASS_CONDITION;
    const LightbarCondition *condition = &item->condition;
    uint32_t groups_count;
    bool bind_groups;

    if ((g_run_state_flags >> RUN_STATE_SCAN_MODE_SHIFT) != 0
        && !lightbar_item_allowed_in_fast_mode(item)) {
        state->bound = 0;
        return 0;
    }

    state->bound = 1;

    switch (item->which_effect) {
    case LightbarEntry_meter_tag:
        state->effect.meter.dirty = 0;
        state->effect.meter.value = rgb_signal_subscribe(&item->effect.meter.signal, effect_subscriber);
        break;

    case LightbarEntry_profile_indicator_tag: {
        SignalRef signal_ref;

        state->effect.profile.dirty = 1;
        signal_ref.is_builtin = true;
        signal_ref.id = RGB_SIGNAL_ACTIVE_PROFILE;
        state->effect.profile.profile = rgb_signal_subscribe(&signal_ref, effect_subscriber);
        signal_ref.id = RGB_SIGNAL_PROFILE_COUNT;
        state->effect.profile.profile_count = rgb_signal_subscribe(&signal_ref, effect_subscriber);
        signal_ref.id = RGB_SIGNAL_PROFILE_COLOR;
        state->effect.profile.profile_color =
            lightbar_argb_from_float_bits(rgb_signal_subscribe(&signal_ref, effect_subscriber));
        rgb_lightbar_resolve_profile_color(&item->effect.profile_indicator, &state->effect.profile);
        break;
    }

    case LightbarEntry_blink_tag:
        state->effect.blink.phase_ms = 0;
        break;

    default:
        break;
    }

    if (!item->has_condition) {
        return 1;
    }

    memset(&state->cond, 0, sizeof(state->cond));

    groups_count = condition->groups_count;
    bind_groups = false;
    if (condition->op == LightbarLogicOp_LIGHTBAR_LOGIC_OP_SINGLE) {
        if (groups_count == 1) {
            /* KNOWN ISSUE: the single group is bound here and once more by the loop below,
             * so every signal it uses carries the subscriber id twice. Matches stock firmware
             * 2.14.1. */
            rgb_lightbar_bind_condition_group(cond_subscriber, &condition->groups[0], state->cond.groups[0]);
            groups_count = condition->groups_count;
            bind_groups = true;
        }
    } else if (condition->op == LightbarLogicOp_LIGHTBAR_LOGIC_OP_AND
               || condition->op == LightbarLogicOp_LIGHTBAR_LOGIC_OP_OR) {
        bind_groups = true;
    }

    if (bind_groups) {
        uint8_t all_bound = 1;

        if (groups_count == 0) {
            state->cond.dirty = 1;
            return 1;
        }
        for (uint32_t group = 0; group < groups_count; group++) {
            all_bound &= (uint8_t)rgb_lightbar_bind_condition_group(
                cond_subscriber | (group << RGB_SUBSCRIBER_GROUP_SHIFT),
                &condition->groups[group],
                state->cond.groups[group]);
            groups_count = condition->groups_count;
        }
        if (all_bound != 0) {
            /* every input is a built-in signal: evaluate on the next frame */
            state->cond.dirty = 1;
        }
    }

    /* Does the tree contain a term that depends on time (trigger, or compare with hold)? */
    for (uint32_t group = 0; group < groups_count; group++) {
        const LightbarConditionGroup *cond_group = &condition->groups[group];

        for (uint32_t term_index = 0; term_index < cond_group->terms_count; term_index++) {
            const LightbarConditionTerm *term = &cond_group->terms[term_index];
            uint8_t term_kind = (uint8_t)term->which_kind;

            if (term_kind == LightbarConditionTerm_trigger_tag) {
                state->cond.has_timed = 1;
                return item->has_condition;
            }
            if (term_kind == LightbarConditionTerm_compare_tag && term->kind.compare.has_hold_ms) {
                state->cond.has_timed = 1;
                return term->kind.compare.has_hold_ms;
            }
        }
    }
    return 1;
}

/*
 * Derives the background colour of a profile-indicator effect (the colour of the part of
 * the range that does not show the active profile) from the profile colour and the
 * background mode. Called at bind time and when the profile colour signal changes.
 */
void rgb_lightbar_resolve_profile_color(const LightbarProfileIndicator *effect_cfg, rgb_lightbar_profile_state_t *effect_state)
{
    effect_state->has_background = 1;

    switch (effect_cfg->background_mode) {
    case LightbarProfileBackground_LIGHTBAR_PROFILE_BACKGROUND_PROFILE_COLOR:
        effect_state->background = effect_state->profile_color;
        break;

    case LightbarProfileBackground_LIGHTBAR_PROFILE_BACKGROUND_NONE:
        effect_state->has_background = 0;
        break;

    case LightbarProfileBackground_LIGHTBAR_PROFILE_BACKGROUND_FIXED_COLOR:
        if (effect_cfg->has_background_color) {
            effect_state->background = lightbar_argb_from_u32(rgb_unpack_color(effect_cfg->background_color));
        }
        break;

    case LightbarProfileBackground_LIGHTBAR_PROFILE_BACKGROUND_CONTRAST_COLOR: {
        /* Hue 0..255 of the profile colour (43 per 60 degrees) picks a fixed contrast colour. */
        uint32_t g = effect_state->profile_color.g;
        uint32_t r = effect_state->profile_color.r;
        uint32_t b = effect_state->profile_color.b;
        uint32_t min;
        int32_t max;
        int32_t delta;
        uint32_t hue;
        uint32_t contrast;

        if (g <= r) {
            min = (b >= g) ? g : b;
        } else {
            min = (b >= r) ? r : b;
        }
        if (g >= r) {
            max = (int32_t)((b < g) ? g : b);
        } else {
            max = (int32_t)((b < r) ? r : b);
        }

        contrast = LIGHTBAR_CONTRAST_YELLOW; /* black and grey */
        if (max != 0) {
            delta = max - (int32_t)min;
            if ((uint8_t)(255 * delta / max) != 0) {
                if ((uint32_t)max == r) {
                    hue = (uint8_t)((int32_t)(43 * (g - b)) / delta);
                } else if ((uint32_t)max == g) {
                    hue = (uint8_t)((int32_t)(43 * (b - r)) / delta + 85);
                } else {
                    hue = (uint8_t)((int32_t)(43 * (r - g)) / delta - 85);
                }

                if (hue <= 14) {
                    contrast = LIGHTBAR_CONTRAST_YELLOW;
                } else if (hue <= 28) {
                    contrast = LIGHTBAR_CONTRAST_RED;
                } else if (hue <= 42) {
                    contrast = LIGHTBAR_CONTRAST_MAGENTA;
                } else if (hue <= 56) {
                    contrast = LIGHTBAR_CONTRAST_BLUE;
                } else if (hue <= 106) {
                    contrast = LIGHTBAR_CONTRAST_CYAN;
                } else if (hue <= 120) {
                    contrast = LIGHTBAR_CONTRAST_RED;
                } else if (hue <= 141) {
                    contrast = LIGHTBAR_CONTRAST_MAGENTA;
                } else if (hue <= 184) {
                    contrast = LIGHTBAR_CONTRAST_GREEN;
                } else if (hue <= 198) {
                    contrast = LIGHTBAR_CONTRAST_RED;
                } else if (hue <= 240) {
                    contrast = LIGHTBAR_CONTRAST_YELLOW;
                } else {
                    contrast = LIGHTBAR_CONTRAST_MINT;
                }
            }
        }
        effect_state->background = lightbar_argb_from_u32(contrast);
        break;
    }

    case LightbarProfileBackground_LIGHTBAR_PROFILE_BACKGROUND_DIMMED_PROFILE_COLOR: {
        uint8_t r = effect_state->profile_color.r >> 1;
        uint8_t g = effect_state->profile_color.g >> 1;

        effect_state->background.b = effect_state->profile_color.b >> 1;
        effect_state->background.g = g;
        effect_state->background.r = r;
        effect_state->background.a = 0xFF;
        break;
    }

    default:
        break;
    }
}

/*
 * Delivers a changed signal value to one light-bar subscriber (id layout: RGB_SUBSCRIBER_*
 * in rgb_types.h). Effect inputs go into the effect state, condition inputs into the state
 * of the addressed term; unchanged values are ignored, changes set the dirty flags that
 * rgb_lightbar_entry_needs_redraw polls.
 */
void rgb_lightbar_notify_signal(uint32_t subscriber, const SignalRef *signal_ref, float value)
{
    uint32_t entry_index = (subscriber >> RGB_SUBSCRIBER_ENTRY_SHIFT) & RGB_SUBSCRIBER_ENTRY_MASK;
    uint8_t subscriber_class = (uint8_t)(subscriber >> 8);
    uint8_t selector = (uint8_t)subscriber;
    const LightbarEntry *entry;
    rgb_lightbar_state_t *state;

    if ((subscriber & RGB_SUBSCRIBER_GLOBAL) != 0) {
        if (!g_rgb_lightbar_config.use_global || g_rgb_global_lightbar_config.entries_count <= entry_index) {
            return;
        }
        entry = &g_rgb_global_lightbar_config.entries[entry_index];
        state = &g_rgb_global_lightbar_states[entry_index];
    } else {
        if (g_rgb_lightbar_config.entries_count <= entry_index) {
            return;
        }
        entry = &g_rgb_lightbar_config.entries[entry_index];
        state = &g_rgb_lightbar_states[entry_index];
    }
    if (state->bound == 0) {
        return;
    }

    if (subscriber_class == (RGB_SUBSCRIBER_CLASS_EFFECT >> 8)) {
        if (entry->which_effect == LightbarEntry_meter_tag) {
            if (value != state->effect.meter.value) {
                state->effect.meter.value = value;
                state->effect.meter.dirty = 1;
            }
        } else if (entry->which_effect == LightbarEntry_profile_indicator_tag) {
            rgb_lightbar_profile_state_t *profile = &state->effect.profile;

            if (signal_ref->id == RGB_SIGNAL_ACTIVE_PROFILE) {
                if (value != profile->profile) {
                    profile->profile = value;
                    profile->dirty = 1;
                }
            } else if (signal_ref->id == RGB_SIGNAL_PROFILE_COUNT) {
                if (value != profile->profile_count) {
                    profile->profile_count = value;
                    profile->dirty = 1;
                }
            } else if (signal_ref->id == RGB_SIGNAL_PROFILE_COLOR) {
                rgb_argb_t color = lightbar_argb_from_float_bits(value);

                /* the alpha byte is not compared */
                if (color.r != profile->profile_color.r
                    || color.g != profile->profile_color.g
                    || color.b != profile->profile_color.b) {
                    profile->profile_color = color;
                    profile->dirty = 1;
                    rgb_lightbar_resolve_profile_color(&entry->effect.profile_indicator, profile);
                }
            }
        }
    } else if (subscriber_class == (RGB_SUBSCRIBER_CLASS_CONDITION >> 8) && entry->has_condition) {
        uint32_t group = selector >> RGB_SUBSCRIBER_GROUP_SHIFT;
        uint32_t term = (selector >> RGB_SUBSCRIBER_COND_SHIFT) & 7;

        /* group 0..1, term 0..1, bit 1 clear */
        if (group <= 1 && (term & 6) == 0 && (selector & 2) == 0) {
            rgb_lightbar_cond_state_t *cond_state = &state->cond.groups[group][term];

            if ((selector & RGB_SUBSCRIBER_RHS) != 0) {
                if (value != cond_state->rhs) {
                    cond_state->rhs = value;
                    cond_state->changed = 1;
                    state->cond.dirty = 1;
                }
            } else if (value != cond_state->lhs) {
                cond_state->lhs = value;
                cond_state->changed = 1;
                state->cond.dirty = 1;
            }
        }
    }
}

/*
 * Sets the brightness of the light-bar LEDs (0 = off). On a change every frame page that
 * holds light-bar channels is queued for re-sending, unless the global fade level is 0.
 */
void rgb_lightbar_set_brightness(uint32_t brightness)
{
    if (g_rgb_lightbar_brightness == brightness) {
        return;
    }
    g_rgb_lightbar_brightness = (uint8_t)brightness;
    if (g_rgb_fade_level != 0) {
        for (uint32_t page = 0; page != RGB_FRAME_PAGE_COUNT; page++) {
            if (g_lightbar_page_channels[page].count != 0) {
                rgb_mark_page_dirty((uint8_t)page);
            }
        }
    }
}

uint8_t lightbar_brightness(void)
{
    return g_rgb_lightbar_brightness;
}

void lightbar_apply_brightness_change(uint32_t key_brightness)
{
    if (g_rgb_lightbar_config.enabled && !g_rgb_lightbar_config.has_brightness) {
        rgb_lightbar_set_brightness(key_brightness);
    }
}

/*
 * Tells rgb_update_frame whether a light-bar entry has to be rendered again in this frame:
 * a hold timer of its condition would run out within elapsed_ms, a condition input changed,
 * or the effect itself asks for it. Returns non-zero for "redraw".
 */
uint32_t rgb_lightbar_entry_needs_redraw(const LightbarEntry *entry, const rgb_lightbar_state_t *state, uint32_t elapsed_ms)
{
    if (state->bound == 0) {
        return 0;
    }

    if (entry->has_condition && (state->cond.dirty != 0 || state->cond.has_timed != 0)) {
        for (uint32_t group = 0; group < entry->condition.groups_count; group++) {
            const LightbarConditionGroup *cond_group = &entry->condition.groups[group];
            const rgb_lightbar_cond_state_t *cond_states = state->cond.groups[group];

            for (uint32_t term = 0; term < cond_group->terms_count; term++) {
                const rgb_lightbar_cond_state_t *cond_state = &cond_states[term];

                if (cond_state->hold_timer_ms != 0 && elapsed_ms > cond_state->hold_timer_ms) {
                    return entry->has_condition;
                }
                if (cond_state->changed != 0) {
                    return cond_state->changed;
                }
            }
        }
    }

    switch (entry->which_effect) {
    case LightbarEntry_meter_tag:
        return state->effect.meter.dirty;
    case LightbarEntry_profile_indicator_tag:
        return state->effect.profile.dirty;
    case LightbarEntry_mirror_tag:
        /* mirrors keys: follow the host while it drives the LEDs directly */
        return g_rgb_sdk_mode_active;
    case LightbarEntry_blink_tag:
        return elapsed_ms != 0;
    default:
        return 0;
    }
}

/* Asks the light-bar entries (global ones first) whether one of them changed. */
bool lightbar_needs_render(uint16_t since_render_ms)
{
    if (g_rgb_lightbar_redraw != 0) {
        return true;
    }
    if (g_rgb_lightbar_active == 0) {
        return false;
    }

    if (g_rgb_lightbar_config.use_global) {
        for (uint8_t i = 0; i < g_rgb_global_lightbar_config.entries_count; ++i) {
            if (rgb_lightbar_entry_needs_redraw(&g_rgb_global_lightbar_config.entries[i],
                                                &g_rgb_global_lightbar_states[i], since_render_ms) != 0) {
                return true;
            }
        }
    }
    for (uint8_t i = 0; i < g_rgb_lightbar_config.entries_count; ++i) {
        if (rgb_lightbar_entry_needs_redraw(&g_rgb_lightbar_config.entries[i], &g_rgb_lightbar_states[i],
                                            since_render_ms) != 0) {
            return true;
        }
    }
    return false;
}

/*
 * Draws all light-bar entries into an LED frame: `glb` entries first (if enabled), then the
 * `lb` entries, so profile entries paint over global ones. When drawing into the main frame
 * the light bar is cleared first.
 */
void rgb_render_lightbar(rgb_frame_t *frame, uint32_t elapsed_ms)
{
    if (frame == &g_rgb_frame_main) {
        for (uint32_t i = 0; i < LIGHTBAR_LED_COUNT; i++) {
            rgb_frame_set_led(frame, k_lightbar_led_ids[i], 0, 0, 0);
        }
    }

    if (g_rgb_lightbar_config.use_global) {
        for (uint8_t i = 0; i < g_rgb_global_lightbar_config.entries_count; i++) {
            rgb_render_lightbar_effect(frame, elapsed_ms,
                                       &g_rgb_global_lightbar_config.entries[i],
                                       &g_rgb_global_lightbar_states[i]);
        }
    }
    for (uint8_t i = 0; i < g_rgb_lightbar_config.entries_count; i++) {
        rgb_render_lightbar_effect(frame, elapsed_ms,
                                   &g_rgb_lightbar_config.entries[i],
                                   &g_rgb_lightbar_states[i]);
    }
}

void lightbar_render_pass(rgb_frame_t *frame, const uint16_t *layer_colors, uint16_t elapsed_ms)
{
    g_rgb_lightbar_redraw = 0;
    if (g_rgb_lightbar_config.enabled) {
        rgb_render_lightbar(frame, elapsed_ms);
    }
    if (g_rgb_sdk_mode_active == 0) {
        rgb_render_overlays(frame, layer_colors, lightbar_overlay_class(), (uint16_t)g_rgb_effect_stamp_ms,
                            elapsed_ms);
    }
}

void lightbar_request_redraw(void)
{
    g_rgb_lightbar_redraw = 1;
}

uint8_t lightbar_overlay_class(void)
{
    return RGB_OVERLAY_CLASS_GLOBAL;
}

void lightbar_draw_overlay(rgb_frame_t *frame, const lightbar_overlay_params_t *params)
{
    for (int i = 0; i != LIGHTBAR_LED_COUNT; ++i) {
        const rgb_argb_t *color = &params->colors[i];

        rgb_frame_set_led(frame, k_lightbar_led_ids[i], color->r, color->g, color->b);
    }
}

void lightbar_clear_overlay(rgb_frame_t *frame)
{
    for (int i = 0; i != LIGHTBAR_LED_COUNT; ++i) {
        rgb_frame_set_led(frame, k_lightbar_led_ids[i], 0, 0, 0);
    }
}
