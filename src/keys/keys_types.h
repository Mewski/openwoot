/*
 * keys_types.h - types and constants of the `keys` module (actuation logic, rapid trigger,
 * advanced keys: dynamic keystroke, mod-tap, toggle, two-key bindings; key action dispatch).
 *
 * The per-key record lives in analog/analog_types.h. Protobuf-backed objects use the generated
 * nanopb types:
 *   g_keys_rapid_trigger_params  rapid-trigger params         RapidTriggerParams
 *   g_keys_rt_groups             file "prt2"                  RapidTriggerGroups
 *   g_keys_advanced_keys         file "ak2"                   AdvancedKeys
 */
#ifndef KEYS_TYPES_H
#define KEYS_TYPES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "proto/settings.pb.h"
#include "analog/analog_types.h"
#include "hid/hid_types.h"

/* ---- Key id ------------------------------------------------------------------------------------ */

/* Key id = row << 5 | col (rows 0..5, columns 0..16), 0xFF = none. */
#define KEYS_KEY_ID(row, col)   ((uint8_t)(((row) << 5) | (col)))
#define KEYS_KEY_ID_ROW(id)     ((uint8_t)((id) >> 5))
#define KEYS_KEY_ID_COL(id)     ((uint8_t)((id) & 0x1Fu))
#define KEYS_KEY_ID_NONE        0xFFu   /* g_keys_last_pressed_key_id while no key is pressed */
#define KEYS_KEY_ID_MAX         0xBFu   /* ids above this are rejected */

/* ---- Distance encoding (actuation points, rapid-trigger sensitivities) ---------------------------- */

/* uint16_t as stored in profiles and in analog_key_state_t.actuation_point. Decoded by
 * keys_get_actuation_threshold and keys_update_press_state, rewritten into the 14-bit form by
 * keys_sanitize_distance. Run-time travel is 12 bit (0..4095). */
#define KEYS_DISTANCE_UNSET         0u       /* per-key: use the profile-wide value */
#define KEYS_DISTANCE_LEGACY_MAX    0xFFu    /* 1..255: 8-bit fraction of the full travel (4095 * v / 255) */
#define KEYS_DISTANCE_Q14_FLAG      0x4000u  /* bits 15:14 == 01: 14-bit fraction in bits 13:0 (4095 * n / 0x3FFF) */
#define KEYS_DISTANCE_Q14_MASK      0x3FFFu
#define KEYS_DISTANCE_IS_Q14(v)     (((v) >> 14) == 1u)

#define KEYS_PRESS_HYSTERESIS       96u      /* fixed actuation: hysteresis that shrinks to 0 the further the key moved past the actuation point */
#define KEYS_RELEASE_FLOOR          16u      /* release / rapid-trigger exit threshold is never below this travel */
#define KEYS_RT_FOLLOW_STEP         32u      /* press_ref_travel follows the travel in steps larger than this */
#define KEYS_TRAVEL_PER_0P1MM_DEFAULT 112u   /* analog_get_key_travel_per_0p1mm for positions without key */

/* g_keys_tap_duration_preset -> tap length in ms: table k_keys_tap_duration_ms {1, 20, 40};
 * presets above 2 mean 40 ms. */
#define KEYS_TAP_PRESET_COUNT       3u
#define KEYS_TAP_MS_FALLBACK        40u

/* ---- Key reference ----------------------------------------------------------------------------- */

/* 8 bytes, built by keys_process_scan_pass from a popped scan event; argument of
 * keys_get_binding_layer, keys_get_actuation_threshold, keys_update_press_state and
 * keys_process_advanced_keys; also stored in the mod-tap slots. */
typedef struct keys_key_ref {
    analog_key_state_t *key_state;  /* +0x00 key record */
    uint8_t key_id;                 /* +0x04 row << 5 | col */
} keys_key_ref_t;

_Static_assert(sizeof(keys_key_ref_t) == 8, "keys_key_ref_t size");
_Static_assert(offsetof(keys_key_ref_t, key_state) == 0, "keys_key_ref_t.key_state");
_Static_assert(offsetof(keys_key_ref_t, key_id) == 4, "keys_key_ref_t.key_id");

/* ---- Special-key (Fn) action state ---------------------------------------------------------------- */

#define KEYS_FN_ACTION_COUNT   15u  /* action codes 0..14 = index of the handler table k_layout_key_action_handlers (layout_types.h) */

/* 2 bytes. Request of one special-key action in the pass context (keys_pass_ctx_t.fn[code],
 * written by keys_apply_key_action) and last dispatched state (g_keys_fn_action_states[code]).
 * Copied as one 16-bit word. */
typedef struct keys_fn_action_state {
    uint8_t key_id;       /* +0 row << 5 | col of the key that carries the action */
    uint8_t layer   : 2;  /* +1 bits 1:0: layer of the binding = third handler argument */
    uint8_t pressed : 1;  /* +1 bit 2: the handler runs when this differs from the stored state */
    uint8_t         : 5;
} keys_fn_action_state_t;

#define KEYS_FN_ACTION_LAYER_MASK   0x03u  /* byte 1 */
#define KEYS_FN_ACTION_PRESSED      0x04u  /* byte 1; 0x0400 in the 16-bit view */

_Static_assert(sizeof(keys_fn_action_state_t) == 2, "keys_fn_action_state_t size");
_Static_assert(offsetof(keys_fn_action_state_t, key_id) == 0, "keys_fn_action_state_t.key_id");

/* ---- Pass context ------------------------------------------------------------------------------ */

/* 0x4A bytes on the stack of keys_process_scan_pass, zeroed at the start of every pass; `ctx`
 * argument of keys_apply_key_action, keys_process_advanced_keys and hid_gamepad_apply_binding. */
typedef struct keys_pass_ctx {
    hid_gamepad_input_t gamepad;                       /* +0x00 gamepad input accumulated this pass (hid/hid_types.h) */
    keys_fn_action_state_t fn[KEYS_FN_ACTION_COUNT];   /* +0x28 special-key requests of this pass (keys_apply_key_action writes, keys_process_scan_pass dispatches) */
    uint16_t elapsed_ms;                               /* +0x46 g_clock_uptime_ms - g_keys_task_stamp_ms */
    uint8_t  tap_ms;                                   /* +0x48 tap length in ms: 1, 20 or 40 */
} keys_pass_ctx_t;

_Static_assert(sizeof(keys_pass_ctx_t) == 0x4A, "keys_pass_ctx_t size");
_Static_assert(offsetof(keys_pass_ctx_t, gamepad) == 0x00, "keys_pass_ctx_t.gamepad");
_Static_assert(offsetof(keys_pass_ctx_t, fn) == 0x28, "keys_pass_ctx_t.fn");
_Static_assert(offsetof(keys_pass_ctx_t, elapsed_ms) == 0x46, "keys_pass_ctx_t.elapsed_ms");
_Static_assert(offsetof(keys_pass_ctx_t, tap_ms) == 0x48, "keys_pass_ctx_t.tap_ms");

/* ---- Rapid trigger -------------------------------------------------------------------------------- */

#define KEYS_RT_GROUP_MAX       30u  /* groups in file "prt2" */
#define KEYS_RT_GROUP_KEYS_MAX  10u  /* key ids per group */

/* oneof tag of a "prt2" group (uint16_t at group +0x0C). */
enum keys_rt_group_kind {
    KEYS_RT_GROUP_PARAMS  = 2,  /* 8-byte rapid-trigger params at group +0x0E; the key records point at them (analog_key_state_t.rt_params) */
    KEYS_RT_GROUP_ENABLED = 3,  /* one bool at group +0x0E: rapid trigger on / off with the profile-wide params */
};

/* Per-key mode in the legacy "prt" message. */
enum keys_rt_legacy_mode {
    KEYS_RT_LEGACY_NO_OVERRIDE = 0,
    KEYS_RT_LEGACY_OFF         = 1,  /* override, rapid trigger off */
    KEYS_RT_LEGACY_ON          = 2,  /* override, rapid trigger on */
};

#define KEYS_LEGACY_MAP_THRESHOLD  50u  /* get-commands answer in map format above this many entries (protocol_cmd_get_legacy_rapid_trigger_keys, protocol_cmd_get_actuation_points) */

/* ---- Advanced keys --------------------------------------------------------------------------------- */

#define KEYS_ADVANCED_KEY_MAX        40u  /* entries in file "ak2" and in g_keys_advanced_key_states */
#define KEYS_ADVANCED_KEY_PAGE_MAX   10u  /* entries per host page message */

/* oneof tag of an advanced-key entry (uint16_t at entry +0). */
enum keys_advanced_key_kind {
    KEYS_AK_DKS       = 1,  /* dynamic keystroke: four bindings driven by four travel events */
    KEYS_AK_MODTAP    = 2,  /* tap key / hold key */
    KEYS_AK_TOGGLE    = 3,  /* tap toggles a latch, long press acts as plain key */
    KEYS_AK_PAIR_DEEP = 4,  /* two keys, the deeper key wins */
    KEYS_AK_PAIR_SOCD = 5,  /* two keys, SOCD resolution */
};

/* Dynamic keystroke: keys_advanced_key_state_t.dks.phase. */
enum keys_dks_phase {
    KEYS_DKS_IDLE     = 0,
    KEYS_DKS_PRESSED  = 1,  /* travel reached the actuation point */
    KEYS_DKS_BOTTOMED = 2,  /* travel reached the bottom-out point */
    KEYS_DKS_LIFTED   = 3,  /* left the bottom, not yet released */
};

/* Dynamic keystroke: 2-bit event per binding in the four event-mask bytes of the entry and in
 * keys_dks_binding_queue_t.events (keys_dks_queue_event, played back by
 * keys_process_advanced_keys). */
enum keys_dks_event {
    KEYS_DKS_EVENT_NONE    = 0,
    KEYS_DKS_EVENT_PRESS   = 1,  /* press and hold */
    KEYS_DKS_EVENT_RELEASE = 2,  /* release (queue: released gap) */
    KEYS_DKS_EVENT_TAP     = 3,  /* pressed for the tap length, then released */
};

#define KEYS_DKS_BINDINGS            4u
#define KEYS_DKS_TIMER_NOT_STARTED   0x3Fu  /* keys_dks_binding_queue_t.timer */
#define KEYS_DKS_DEFAULT_BOTTOM      3693u  /* bottom-out point in travel units when the entry has none */

/* Dynamic keystroke: output queue of one binding (2 bytes). */
typedef struct keys_dks_binding_queue {
    uint8_t events;       /* +0 shift register, 2 bits per step (enum keys_dks_event), current step in bits 1:0 */
    uint8_t pending : 2;  /* +1 bits 1:0: number of steps queued behind the current one */
    uint8_t timer   : 6;  /* +1 bits 7:2: countdown in ms, 0x3F = not started (loaded with the tap length) */
} keys_dks_binding_queue_t;

_Static_assert(sizeof(keys_dks_binding_queue_t) == 2, "keys_dks_binding_queue_t size");

/* Mod-tap: keys_advanced_key_state_t.modtap.state. */
enum keys_modtap_state {
    KEYS_MODTAP_IDLE      = 0,
    KEYS_MODTAP_UNDECIDED = 1,  /* pressed, hold time not reached */
    KEYS_MODTAP_HOLD      = 2,  /* hold key code active */
    KEYS_MODTAP_TAP       = 3,  /* tap key code active for the tap length */
};

/* Mod-tap: optional interrupt mode of the entry (payload byte 5 when byte 4 is set). */
enum keys_modtap_interrupt_mode {
    KEYS_MODTAP_INTERRUPT_NEVER = 1,  /* other keys do not resolve the mod-tap */
    KEYS_MODTAP_INTERRUPT_HOLD  = 2,  /* another key press resolves to hold */
    KEYS_MODTAP_INTERRUPT_TAP   = 3,  /* another key press resolves to tap */
};

#define KEYS_MODTAP_DEFAULT_HOLD_MS  200u    /* hold time when the entry has none */
#define KEYS_MODTAP_HOLD_UNIT_MS     10u     /* unit of the entry's hold time byte */
#define KEYS_MODTAP_TIMER_IDLE       0xFFFFu /* modtap.timer_ms: state just entered, timer not started */
#define KEYS_TOGGLE_TAP_MAX_MS       199u    /* toggle: a press up to this long flips the latch */

/* Two-key bindings: resolution mode (payload byte 1). Any other value: both off. */
enum keys_pair_mode {
    KEYS_PAIR_MODE_BOTH       = 0,  /* both keys on */
    KEYS_PAIR_MODE_NEITHER    = 1,
    KEYS_PAIR_MODE_KEY1       = 2,
    KEYS_PAIR_MODE_KEY2       = 3,
    KEYS_PAIR_MODE_LAST_PRESS = 4,  /* the key pressed last wins (pair.last_pressed) */
};

#define KEYS_PAIR_FULL_TRAVEL        0xFADu  /* travel above this counts as fully pressed */
#define KEYS_PAIR_SAME_DEPTH         159u    /* kind 4: travel difference up to this keeps the previous decision */

/* Run-time state of one advanced key, 12 bytes, same index as the entry in g_keys_advanced_keys.
 * Zeroed when the entry is (re)written. The layout depends on the entry kind. */
typedef union keys_advanced_key_state {
    uint8_t raw[12];

    struct {                                /* KEYS_AK_DKS */
        uint8_t phase;                      /* +0x00 enum keys_dks_phase */
        keys_dks_binding_queue_t queue[KEYS_DKS_BINDINGS]; /* +0x01 output queue per binding */
    } dks;

    struct {                                /* KEYS_AK_MODTAP */
        uint8_t  state;                     /* +0x00 enum keys_modtap_state */
        uint16_t timer_ms;                  /* +0x02 0xFFFF = not started; advanced by ctx->elapsed_ms */
        int8_t   slot;                      /* +0x04 index into g_keys_modtap_slots, -1 = none */
    } modtap;

    struct {                                /* KEYS_AK_TOGGLE */
        uint8_t  pressed;                   /* +0x00 key held */
        uint8_t  latched;                   /* +0x01 latch on: output stays pressed with travel 4095; pins the entry's layer (keys_get_binding_layer), owns the overlay whose id is the key id (keys_stop_toggle_overlays) */
        uint16_t hold_ms;                   /* +0x02 press time, counted while pressed */
    } toggle;

    struct {                                /* KEYS_AK_PAIR_DEEP / KEYS_AK_PAIR_SOCD */
        struct {
            uint8_t  pressed;               /* +0x00 / +0x04 */
            uint16_t travel;                /* +0x02 / +0x06 */
        } key[2];                           /* [0] = the entry's own key, [1] = the second key */
        uint8_t last_pressed;               /* +0x08 1 = key 1, 2 = key 2 */
        uint8_t out1_or_latch;              /* +0x09 kind 4: previous decision for key 1; kind 5: latch "both keys were fully pressed" */
        uint8_t out2;                       /* +0x0A kind 4: previous decision for key 2 */
    } pair;
} keys_advanced_key_state_t;

_Static_assert(sizeof(keys_advanced_key_state_t) == 12, "keys_advanced_key_state_t size");
_Static_assert(offsetof(keys_advanced_key_state_t, dks.queue) == 1, "ak_state.dks.queue");
_Static_assert(offsetof(keys_advanced_key_state_t, modtap.timer_ms) == 2, "ak_state.modtap.timer_ms");
_Static_assert(offsetof(keys_advanced_key_state_t, modtap.slot) == 4, "ak_state.modtap.slot");
_Static_assert(offsetof(keys_advanced_key_state_t, toggle.latched) == 1, "ak_state.toggle.latched");
_Static_assert(offsetof(keys_advanced_key_state_t, toggle.hold_ms) == 2, "ak_state.toggle.hold_ms");
_Static_assert(offsetof(keys_advanced_key_state_t, pair.key[0].travel) == 2, "ak_state.pair.key[0].travel");
_Static_assert(offsetof(keys_advanced_key_state_t, pair.key[1].pressed) == 4, "ak_state.pair.key[1].pressed");
_Static_assert(offsetof(keys_advanced_key_state_t, pair.key[1].travel) == 6, "ak_state.pair.key[1].travel");
_Static_assert(offsetof(keys_advanced_key_state_t, pair.last_pressed) == 8, "ak_state.pair.last_pressed");
_Static_assert(offsetof(keys_advanced_key_state_t, pair.out1_or_latch) == 9, "ak_state.pair.out1_or_latch");
_Static_assert(offsetof(keys_advanced_key_state_t, pair.out2) == 10, "ak_state.pair.out2");

/* ---- Pending mod-tap slots ------------------------------------------------------------------------- */

#define KEYS_MODTAP_SLOT_COUNT  5u

/* 12 bytes. g_keys_modtap_slots[5]: mod-tap keys that another key press may resolve.
 * Allocation writes `flags & 0xF0 | 3` and copies the key ref; keys_modtap_release_slot clears
 * bit 0. */
typedef struct keys_modtap_slot {
    union {
        uint8_t flags;                       /* +0x00 */
        struct {
            uint8_t in_use          : 1;     /* bit 0 */
            uint8_t free_on_resolve : 1;     /* bit 1: release the slot when another key press resolved it */
            uint8_t unk_bits_2_3    : 2;     /* bits 3:2: must be 0 for the slot to be considered; never set by this firmware */
            uint8_t                 : 4;
        };
    };
    keys_key_ref_t key;                      /* +0x04 the mod-tap key: record pointer and key id at +0x08 */
} keys_modtap_slot_t;

#define KEYS_MODTAP_SLOT_IN_USE           0x01u
#define KEYS_MODTAP_SLOT_FREE_ON_RESOLVE  0x02u
#define KEYS_MODTAP_SLOT_UNK_MASK         0x0Cu

_Static_assert(sizeof(keys_modtap_slot_t) == 12, "keys_modtap_slot_t size");
_Static_assert(offsetof(keys_modtap_slot_t, flags) == 0, "keys_modtap_slot_t.flags");
_Static_assert(offsetof(keys_modtap_slot_t, key) == 4, "keys_modtap_slot_t.key");
_Static_assert(offsetof(keys_modtap_slot_t, key.key_id) == 8, "keys_modtap_slot_t.key.key_id");

#endif /* KEYS_TYPES_H */
