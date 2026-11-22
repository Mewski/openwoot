/*
 * lightbar_internal.h - what the source files of feature `lightbar` share: its RAM objects, the
 * LED table and the functions of configuration, binding, conditions and effects.
 *
 * An entry of `lb` or `glb` is an optional condition tree (lightbar_condition.c) plus one effect
 * (lightbar_effects.c); lightbar.c loads, binds and draws the entries.
 *
 * Depends on rgb/lightbar/lightbar_types.h and the rgb module (frame, data).
 */
#ifndef RGB_LIGHTBAR_LIGHTBAR_INTERNAL_H
#define RGB_LIGHTBAR_LIGHTBAR_INTERNAL_H

#include <stdbool.h>
#include <stdint.h>

#include "proto/settings.pb.h"
#include "rgb/lightbar/lightbar.h"
#include "rgb/lightbar/lightbar_types.h"
#include "rgb/rgb_data.h"

/* ---- RAM objects (lightbar_data.c) ------------------------------------------------------- */

extern LightbarConfig g_rgb_lightbar_config;             /* file `lb` of the active profile */
extern GlobalLightbarConfig g_rgb_global_lightbar_config; /* root file `glb` */

extern rgb_lightbar_state_t g_rgb_lightbar_states[RGB_LIGHTBAR_ENTRY_MAX];        /* state of the `lb` entries */
extern rgb_lightbar_state_t g_rgb_global_lightbar_states[RGB_LIGHTBAR_ENTRY_MAX]; /* state of the `glb` entries */

extern uint8_t g_rgb_lightbar_redraw;     /* 1 = draw the light bar on the next frame regardless of the entries */
extern uint8_t g_rgb_lightbar_active;     /* OR of the rgb_lightbar_init_item_state results: at least one entry is bound */
extern uint8_t g_rgb_lightbar_brightness; /* brightness of the light-bar LEDs 0..255, 0 = off */

extern lightbar_page_channels_t g_lightbar_page_channels[RGB_FRAME_PAGE_COUNT]; /* light-bar channels per frame page */

/* ---- LEDs (lightbar_leds.c) ---------------------------------------------------------------- */

extern const uint8_t k_lightbar_led_ids[LIGHTBAR_LED_COUNT]; /* LED number of each light-bar position */

/* ---- configuration, binding, drawing (lightbar.c) ------------------------------------------ */

void rgb_load_global_lightbar_config(void);
/* Overwrites root file `glb` with the encoded message (the encode result is not checked; a failed
 * encode writes the partial output). The caller checks that the file system is mounted. */
void lightbar_write_global_file(const GlobalLightbarConfig *config);
/* The entries in RAM changed: redraw, and bind them again (or switch the bar off if the profile
 * has it disabled). */
void lightbar_config_changed(void);
void rgb_lightbar_rebind_signals(const RgbSettings *rgb_settings);
uint8_t rgb_lightbar_init_item_state(uint32_t subscriber_id, const LightbarEntry *item, rgb_lightbar_state_t *state);
void rgb_lightbar_resolve_profile_color(const LightbarProfileIndicator *effect_cfg, rgb_lightbar_profile_state_t *effect_state);
void rgb_lightbar_set_brightness(uint32_t brightness);
uint32_t rgb_lightbar_entry_needs_redraw(const LightbarEntry *entry, const rgb_lightbar_state_t *state, uint32_t elapsed_ms);
void rgb_render_lightbar(rgb_frame_t *frame, uint32_t elapsed_ms);

/* ---- conditions (lightbar_condition.c) -------------------------------------------------------- */

uint32_t rgb_lightbar_eval_condition(const LightbarConditionTerm *cond, rgb_lightbar_cond_state_t *cond_state, uint32_t elapsed_ms);
int rgb_lightbar_bind_compare_condition(uint32_t subscriber, const LightbarConditionTerm *condition, rgb_lightbar_cond_state_t *cond_state);
int rgb_lightbar_bind_condition_group(uint32_t subscriber, const LightbarConditionGroup *group, rgb_lightbar_cond_state_t *cond_states);
int rgb_lightbar_eval_condition_group(const LightbarConditionGroup *group, rgb_lightbar_cond_state_t *cond_states, uint32_t elapsed_ms);
int rgb_lightbar_group_allowed_in_fast_mode(const LightbarConditionGroup *group);

/* ---- effects (lightbar_effects.c) -------------------------------------------------------------- */

void rgb_render_lightbar_effect(rgb_frame_t *frame, uint32_t elapsed_ms, const LightbarEntry *entry, rgb_lightbar_state_t *state);

/* ---- host protocol (lightbar_protocol.c) ------------------------------------------------------ */

/* Event 2 to the host, if it subscribed to it: the entries were bound again, so the list of host
 * signals may have changed. */
void protocol_send_lightbar_changed_event(void);

#endif /* RGB_LIGHTBAR_LIGHTBAR_INTERNAL_H */
