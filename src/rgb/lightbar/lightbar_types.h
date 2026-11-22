/*
 * lightbar_types.h - data model of feature `lightbar`: the channels of the light-bar LEDs per
 * frame page, the subscriber ids of the light-bar entries and the run-time state of one entry.
 *
 * The value sets of the light-bar configuration are generated nanopb names and are not repeated
 * here: effect kind = oneof tag of LightbarEntry (2 meter, 3 profile indicator, 4 mirror,
 * 5 blink, 6 static colour, 7 custom colours); term kind = oneof tag of the condition term
 * (1 trigger, 2 compare); operand kind = oneof tag of the operand (1 signal, 2 constant); logic
 * operator, comparator and profile-indicator background mode are generated enums.
 *
 * The layouts match stock firmware 2.14.1 (stored files `lb` and `glb`, offsets used by the
 * code of the feature).
 *
 * Depends on rgb/lightbar/lightbar.h, rgb/signals/signals.h (subscriber kinds, signal ids) and
 * the light-bar constants of board.h.
 */
#ifndef RGB_LIGHTBAR_LIGHTBAR_TYPES_H
#define RGB_LIGHTBAR_LIGHTBAR_TYPES_H

#include <stddef.h>
#include <stdint.h>

#include "proto/settings.pb.h"
#include "rgb/lightbar/lightbar.h"
#include "rgb/signals/signals.h"

#define RGB_LIGHTBAR_ENTRY_MAX     4    /* entries per `lb` / `glb` file */

/*
 * Channels of the light-bar LEDs that live in one frame page (g_lightbar_page_channels, filled
 * at start-up from k_lightbar_led_ids). The flush step of rgb_update_frame uses the light-bar
 * brightness instead of the key brightness for these pwm[] offsets.
 */
#define LIGHTBAR_PAGE_CHANNEL_MAX  30

typedef struct {
    uint8_t count;                               /* 0x00 used entries of channels[] (3 per LED) */
    uint8_t channels[LIGHTBAR_PAGE_CHANNEL_MAX]; /* 0x01 offsets into rgb_frame_page_t.pwm[] */
} lightbar_page_channels_t;

_Static_assert(sizeof(lightbar_page_channels_t) == 31, "lightbar_page_channels_t size"); /* one list per frame page */
_Static_assert(offsetof(lightbar_page_channels_t, channels) == 1, "lightbar_page_channels_t.channels");

/*
 * Subscriber id of a light-bar entry (rgb_lightbar_init_item_state, rgb_lightbar_notify_signal):
 * kind RGB_SUBSCRIBER_KIND_LIGHTBAR in bits 31:24 (signals.h), the rest as below.
 */
#define RGB_SUBSCRIBER_GLOBAL          0x00800000u /* entry of `glb` instead of `lb` */
#define RGB_SUBSCRIBER_ENTRY_SHIFT     16          /* bits 22:16 entry index 0..3 */
#define RGB_SUBSCRIBER_ENTRY_MASK      0x7Fu
#define RGB_SUBSCRIBER_CLASS_EFFECT    0x00000100u /* byte 1 = 1: input of the effect */
#define RGB_SUBSCRIBER_CLASS_CONDITION 0x00000200u /* byte 1 = 2: input of a condition */
#define RGB_SUBSCRIBER_GROUP_SHIFT     5           /* class 2: bits 7:5 group index 0..1 */
#define RGB_SUBSCRIBER_COND_SHIFT      2           /* class 2: bits 4:2 condition index 0..1 */
#define RGB_SUBSCRIBER_RHS             0x00000001u /* class 2: right operand instead of left operand / trigger */
#define RGB_SUBSCRIBER_LIGHTBAR(entry) \
    ((RGB_SUBSCRIBER_KIND_LIGHTBAR << RGB_SUBSCRIBER_KIND_SHIFT) | ((uint32_t)(entry) << RGB_SUBSCRIBER_ENTRY_SHIFT))

/*
 * Built-in signals a condition may reference while the scan mode level is non-zero
 * (rgb_lightbar_group_allowed_in_fast_mode, rgb_lightbar_init_item_state).
 */
#define RGB_SIGNAL_FAST_MODE_MASK  0x5F9

/*
 * Run-time state of one condition, 16 bytes (rgb_lightbar_eval_condition,
 * rgb_lightbar_bind_compare_condition, rgb_lightbar_notify_signal).
 */
typedef struct {
    uint8_t  active;        /* 0x00 current result */
    uint8_t  changed;       /* 0x01 an input value arrived; consumed by rgb_lightbar_eval_condition */
    uint8_t  _pad_02[2];
    float    lhs;           /* 0x04 value of the condition's signal */
    float    rhs;           /* 0x08 value of the operand signal (operand oneof tag 1) */
    uint16_t hold_timer_ms; /* 0x0C counts down by the frame time */
    uint8_t  _pad_0e[2];
} rgb_lightbar_cond_state_t;

_Static_assert(sizeof(rgb_lightbar_cond_state_t) == 16, "rgb_lightbar_cond_state_t size");
_Static_assert(offsetof(rgb_lightbar_cond_state_t, changed) == 0x01, "cond_state.changed");
_Static_assert(offsetof(rgb_lightbar_cond_state_t, lhs) == 0x04, "cond_state.lhs");
_Static_assert(offsetof(rgb_lightbar_cond_state_t, rhs) == 0x08, "cond_state.rhs");
_Static_assert(offsetof(rgb_lightbar_cond_state_t, hold_timer_ms) == 0x0C, "cond_state.hold_timer_ms");

/* Kind 2 (meter): offsets relative to the entry state. */
typedef struct {
    float   value; /* 0x48 signal value (rgb_lightbar_notify_signal) */
    uint8_t dirty; /* 0x4C redraw flag, cleared after drawing (rgb_render_lightbar_effect, rgb_lightbar_entry_needs_redraw) */
} rgb_lightbar_meter_state_t;

/* Kind 3 (profile indicator): argument effect_state of rgb_lightbar_resolve_profile_color. */
typedef struct {
    float      profile;        /* 0x48 signal 0: (index + 1) | namespace << 8 */
    float      profile_count;  /* 0x4C signal 6 */
    uint8_t    dirty;          /* 0x50 redraw flag, cleared after drawing */
    rgb_argb_t profile_color;  /* 0x51 raw bits of signal 10 (unaligned) */
    uint8_t    has_background; /* 0x55 0 when background mode == 1 (rgb_lightbar_resolve_profile_color) */
    rgb_argb_t background;     /* 0x56 colour of the rest of the range (unaligned, rgb_render_lightbar_effect) */
} rgb_lightbar_profile_state_t;

/* Kind 5 (blink). */
typedef struct {
    uint16_t phase_ms; /* 0x48 (phase + elapsed) % period */
} rgb_lightbar_blink_state_t;

/* Condition tree state of one entry, cleared as one 68-byte block at bind (rgb_lightbar_init_item_state). */
typedef struct {
    uint8_t result;    /* 0x04 last result of the condition tree */
    uint8_t dirty;     /* 0x05 a condition input changed (or all inputs were bound), re-evaluate; cleared by rgb_render_lightbar_effect */
    uint8_t has_timed; /* 0x06 a condition is a trigger or a compare with hold time (rgb_lightbar_init_item_state) */
    uint8_t _pad_07;
    rgb_lightbar_cond_state_t groups[2][2]; /* 0x08 [group][condition] */
} rgb_lightbar_tree_state_t;

/*
 * Run-time state of one light-bar entry, 92 bytes (rgb_lightbar_init_item_state,
 * rgb_render_lightbar_effect, rgb_lightbar_notify_signal, rgb_lightbar_entry_needs_redraw).
 */
typedef struct {
    uint8_t bound;       /* 0x00 1 = entry accepted by rgb_lightbar_init_item_state; 0 = ignored everywhere */
    uint8_t _pad_01[3];
    rgb_lightbar_tree_state_t cond; /* 0x04 */
    union {              /* 0x48 */
        rgb_lightbar_meter_state_t   meter;   /* kind 2 */
        rgb_lightbar_profile_state_t profile; /* kind 3 */
        rgb_lightbar_blink_state_t   blink;   /* kind 5 */
        uint8_t                      raw[20];
    } effect;
} rgb_lightbar_state_t;

_Static_assert(sizeof(rgb_lightbar_state_t) == 92, "rgb_lightbar_state_t size");
_Static_assert(offsetof(rgb_lightbar_state_t, cond) == 0x04, "lightbar_state.cond");
_Static_assert(offsetof(rgb_lightbar_state_t, cond.dirty) == 0x05, "lightbar_state.cond.dirty");
_Static_assert(offsetof(rgb_lightbar_state_t, cond.has_timed) == 0x06, "lightbar_state.cond.has_timed");
_Static_assert(offsetof(rgb_lightbar_state_t, cond.groups) == 0x08, "lightbar_state.cond.groups");
_Static_assert(offsetof(rgb_lightbar_state_t, cond.groups[1]) == 0x28, "lightbar_state.cond.groups[1]");
_Static_assert(offsetof(rgb_lightbar_state_t, effect) == 0x48, "lightbar_state.effect");
_Static_assert(offsetof(rgb_lightbar_state_t, effect.meter.dirty) == 0x4C, "lightbar_state.meter.dirty");
_Static_assert(offsetof(rgb_lightbar_state_t, effect.profile.profile_count) == 0x4C, "lightbar_state.profile.profile_count");
_Static_assert(offsetof(rgb_lightbar_state_t, effect.profile.dirty) == 0x50, "lightbar_state.profile.dirty");
_Static_assert(offsetof(rgb_lightbar_state_t, effect.profile.profile_color) == 0x51, "lightbar_state.profile.profile_color");
_Static_assert(offsetof(rgb_lightbar_state_t, effect.profile.has_background) == 0x55, "lightbar_state.profile.has_background");
_Static_assert(offsetof(rgb_lightbar_state_t, effect.profile.background) == 0x56, "lightbar_state.profile.background");

/* Sizes of the generated configuration types; the stored files depend on them. */
_Static_assert(sizeof(LightbarConfig) == 0x33C, "lb file size");
_Static_assert(sizeof(GlobalLightbarConfig) == 0x334, "glb file size");
_Static_assert(sizeof(LightbarEntry) == 204, "light-bar entry size");

#endif /* RGB_LIGHTBAR_LIGHTBAR_TYPES_H */
