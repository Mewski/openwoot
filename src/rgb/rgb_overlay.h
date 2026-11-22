/*
 * rgb_overlay.h - overlays: timed indicators drawn on top of the frame (single keys, the global
 * brightness fade, host light-bar colours). Data model of the overlay slots (g_rgb_overlay_slots)
 * and the functions of rgb_overlay.c and rgb_overlay_lock.c.
 *
 * The record layout matches stock firmware 2.14.1 of each board: with feature `lightbar` the
 * union holds the light-bar colours (10 LEDs: record 42 bytes, slot 52), without it the record
 * is 16 bytes and the slot 28. The sizes are asserted as formulas over LIGHTBAR_LED_COUNT.
 *
 * Depends on rgb/rgb_types.h and on the public header of feature `lightbar` (its overlay type).
 */
#ifndef RGB_OVERLAY_H
#define RGB_OVERLAY_H

#include <stddef.h>
#include <stdint.h>

#include "rgb/lightbar/lightbar.h"
#include "rgb/rgb_types.h"

#define RGB_OVERLAY_SLOT_COUNT     5
#define RGB_OVERLAY_DURATION_FOREVER 0xFFFF /* until rgb_overlay_stop; never decremented by rgb_render_overlays */

/* params.type (slot +0x0A), switch in rgb_render_overlays. */
enum rgb_overlay_type {
    RGB_OVERLAY_SOLID_KEY       = 0, /* one key in one colour */
    RGB_OVERLAY_BLINK_KEY       = 1, /* one key alternating between two colours */
    RGB_OVERLAY_FADE            = 2, /* ramps g_rgb_fade_level */
    RGB_OVERLAY_LIGHTBAR_COLORS = 3  /* light-bar colours set by the host (feature lightbar) */
};

/* Redraw class (slot +0, bits of g_rgb_overlay_dirty_mask and of the class_mask argument). */
#define RGB_OVERLAY_CLASS_KEYS     0x01 /* types 0, 1 */
#define RGB_OVERLAY_CLASS_GLOBAL   0x02 /* type 2, and type 3 with feature lightbar (lightbar_overlay_class) */
#define RGB_OVERLAY_CLASS_ALL      (RGB_OVERLAY_CLASS_KEYS | RGB_OVERLAY_CLASS_GLOBAL)

/*
 * Overlay ids (slot +1). Any other id is a key position (RGB_KEY_POS) used by the per-key
 * toggle overlays of layout_key_action_toggle_key_disable and keys_process_scan_pass.
 */
enum rgb_overlay_id {
    RGB_OVERLAY_ID_NONE           = 0x00, /* rgb_overlay_stop(0) is a no-op */
    RGB_OVERLAY_ID_PROFILE_SWITCH = 0x0A, /* blink of the profile key, 1000 ms (layout_request_profile_switch) */
    RGB_OVERLAY_ID_CAPS_LOCK      = 0x14, /* k_rgb_lock_indicator_overlay_ids, rgb_set_lock_indicator */
    RGB_OVERLAY_ID_LAYER_LOCK     = 0x15,
    RGB_OVERLAY_ID_SCROLL_LOCK    = 0x16,
    RGB_OVERLAY_ID_NUM_LOCK       = 0x17,
    RGB_OVERLAY_ID_HOST_LIGHTBAR  = 0x1D, /* protocol_report_set_lightbar_colors */
    RGB_OVERLAY_ID_IDLE_FADE      = 0x1E, /* rgb_update_frame, rgb_reset_idle_timer */
    RGB_OVERLAY_ID_WAKE_FADE      = 0x1F  /* rgb_wake_led_driver, 250 ms */
};

/* Argument of rgb_set_lock_indicator. */
enum rgb_lock_indicator {
    RGB_INDICATOR_CAPS_LOCK   = 0,
    RGB_INDICATOR_SCROLL_LOCK = 1,
    RGB_INDICATOR_NUM_LOCK    = 2,
    RGB_INDICATOR_LAYER_LOCK  = 3
};

/* Overlay type 0: one key in one colour. Offsets in the comments are slot offsets. */
typedef struct {
    rgb_argb_t color;   /* 0x0C b, g, r (alpha unused) */
    uint8_t    key_pos; /* 0x10 RGB_KEY_POS */
} rgb_overlay_solid_key_t;

/* Overlay type 1: one key alternating between two colours. */
typedef struct {
    rgb_argb_t color_a;   /* 0x0C shown while phase <= period / 2 */
    rgb_argb_t color_b;   /* 0x10 shown while phase > period / 2 */
    uint8_t    key_pos;   /* 0x14 RGB_KEY_POS */
    uint8_t    _pad_0b;   /* 0x15 */
    uint16_t   period_ms; /* 0x16 a period of 0 is not guarded (rgb_render_overlays loops forever with interrupts masked) */
    uint16_t   phase_ms;  /* 0x18 run-time */
} rgb_overlay_blink_key_t;

/* Overlay type 2: ramp of g_rgb_fade_level. */
typedef struct {
    uint8_t  from;       /* 0x0C start value of g_rgb_fade_level */
    uint8_t  to;         /* 0x0D end value, kept until the slot ends */
    uint16_t fade_ms;    /* 0x0E */
    uint16_t elapsed_ms; /* 0x10 run-time; level = from + (to - from) * (255 * elapsed / fade_ms) / 256 */
} rgb_overlay_fade_t;

/*
 * Overlay record: argument `params` of rgb_overlay_start, copied into the slot at +0x0A.
 * Offsets in the comments are slot offsets.
 */
typedef struct {
    uint8_t type;    /* 0x0A enum rgb_overlay_type */
    uint8_t _pad_01; /* 0x0B */
    union {
        rgb_overlay_solid_key_t   solid_key; /* 0x0C type 0 */
        rgb_overlay_blink_key_t   blink_key; /* 0x0C type 1 */
        rgb_overlay_fade_t        fade;      /* 0x0C type 2 */
        lightbar_overlay_params_t lightbar;  /* 0x0C type 3 (feature lightbar) */
    } u;             /* 0x0C */
} rgb_overlay_params_t;

/* Union of the record: the largest member, which is the blink overlay (14 bytes) or the
 * light-bar colours (4 bytes per light-bar LED). */
#define RGB_OVERLAY_UNION_KEYS_SIZE 14u
#define RGB_OVERLAY_UNION_LIGHTBAR_SIZE (LIGHTBAR_LED_COUNT * sizeof(rgb_argb_t))
#define RGB_OVERLAY_UNION_SIZE                                                                     \
    (RGB_OVERLAY_UNION_LIGHTBAR_SIZE > RGB_OVERLAY_UNION_KEYS_SIZE ? RGB_OVERLAY_UNION_LIGHTBAR_SIZE \
                                                                    : RGB_OVERLAY_UNION_KEYS_SIZE)
/* Size of the record (type, pad byte, union) and of the slot (10 header bytes and the record,
 * rounded up to the 4-byte alignment of start_tick). */
#define RGB_OVERLAY_PARAMS_SIZE    (2u + RGB_OVERLAY_UNION_SIZE)
#define RGB_OVERLAY_SLOT_SIZE      ((0x0Au + RGB_OVERLAY_PARAMS_SIZE + 3u) / 4u * 4u)

_Static_assert(sizeof(rgb_overlay_blink_key_t) == RGB_OVERLAY_UNION_KEYS_SIZE, "rgb_overlay_blink_key_t size");
_Static_assert(sizeof(lightbar_overlay_params_t) == RGB_OVERLAY_UNION_LIGHTBAR_SIZE, "lightbar_overlay_params_t size");
_Static_assert(sizeof(rgb_overlay_params_t) == RGB_OVERLAY_PARAMS_SIZE, "rgb_overlay_params_t size"); /* memset in every caller */
_Static_assert(_Alignof(rgb_overlay_params_t) == 2, "rgb_overlay_params_t alignment");
_Static_assert(offsetof(rgb_overlay_params_t, u) == 2, "rgb_overlay_params_t.u");
_Static_assert(offsetof(rgb_overlay_params_t, u.solid_key.key_pos) == 6, "solid_key.key_pos");
_Static_assert(offsetof(rgb_overlay_params_t, u.blink_key.color_b) == 6, "blink_key.color_b");
_Static_assert(offsetof(rgb_overlay_params_t, u.blink_key.key_pos) == 10, "blink_key.key_pos");
_Static_assert(offsetof(rgb_overlay_params_t, u.blink_key.period_ms) == 12, "blink_key.period_ms");
_Static_assert(offsetof(rgb_overlay_params_t, u.blink_key.phase_ms) == 14, "blink_key.phase_ms");
_Static_assert(offsetof(rgb_overlay_params_t, u.fade.to) == 3, "fade.to");
_Static_assert(offsetof(rgb_overlay_params_t, u.fade.fade_ms) == 4, "fade.fade_ms");
_Static_assert(offsetof(rgb_overlay_params_t, u.fade.elapsed_ms) == 6, "fade.elapsed_ms");

/* One overlay slot (rgb_overlay_start, rgb_overlay_stop, rgb_render_overlays). */
typedef struct {
    uint8_t  redraw_class; /* 0x00 RGB_OVERLAY_CLASS_*: 1 for types 0/1, 2 for types 2/3, else 0 */
    uint8_t  id;           /* 0x01 enum rgb_overlay_id or key position; lookup key */
    uint8_t  restart;      /* 0x02 set by rgb_overlay_start; the first render pass uses now - start_tick as elapsed time */
    uint8_t  _pad_03;
    uint32_t start_tick;   /* 0x04 g_clock_uptime_ms at start */
    uint16_t duration_ms;  /* 0x08 remaining time; 0xFFFF = until stopped; 0 = finished, slot removed on the next pass */
    rgb_overlay_params_t params; /* 0x0A */
} rgb_overlay_slot_t;

_Static_assert(sizeof(rgb_overlay_slot_t) == RGB_OVERLAY_SLOT_SIZE, "rgb_overlay_slot_t size");
_Static_assert(offsetof(rgb_overlay_slot_t, id) == 0x01, "rgb_overlay_slot_t.id");
_Static_assert(offsetof(rgb_overlay_slot_t, restart) == 0x02, "rgb_overlay_slot_t.restart");
_Static_assert(offsetof(rgb_overlay_slot_t, start_tick) == 0x04, "rgb_overlay_slot_t.start_tick");
_Static_assert(offsetof(rgb_overlay_slot_t, duration_ms) == 0x08, "rgb_overlay_slot_t.duration_ms");
_Static_assert(offsetof(rgb_overlay_slot_t, params) == 0x0A, "rgb_overlay_slot_t.params");
_Static_assert(offsetof(rgb_overlay_slot_t, params.u.solid_key.key_pos) == 0x10, "slot solid key_pos");
_Static_assert(offsetof(rgb_overlay_slot_t, params.u.blink_key.key_pos) == 0x14, "slot blink key_pos");
_Static_assert(offsetof(rgb_overlay_slot_t, params.u.blink_key.period_ms) == 0x16, "slot blink period");
_Static_assert(offsetof(rgb_overlay_slot_t, params.u.blink_key.phase_ms) == 0x18, "slot blink phase");
_Static_assert(offsetof(rgb_overlay_slot_t, params.u.fade.fade_ms) == 0x0E, "slot fade_ms");
_Static_assert(offsetof(rgb_overlay_slot_t, params.u.fade.elapsed_ms) == 0x10, "slot fade elapsed");

void rgb_overlay_start(uint32_t overlay_id, uint16_t duration, const rgb_overlay_params_t *params);
void rgb_overlay_stop(uint32_t overlay_id);
void rgb_render_overlays(rgb_frame_t *frame, const uint16_t *layer_colors, uint8_t class_mask, uint32_t now_ms, uint16_t elapsed_ms);
void rgb_set_lock_indicator(uint32_t indicator, uint32_t on);
void rgb_update_lock_indicators(void);

#endif /* RGB_OVERLAY_H */
