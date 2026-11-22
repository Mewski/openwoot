/*
 * keys_scan_pass_internal.h - shared state and helpers of the files that make up
 * keys_process_scan_pass (keys_scan_pass*.c). Not a public header.
 *
 * The key pipeline is one long pass, split into the steps S0..S14 (listed in keys_scan_pass.c)
 * and over four files:
 *
 *   keys_scan_pass.c           S0..S3, S5..S7, S12, S14 and keys_process_scan_pass itself
 *   keys_scan_pass_advanced.c  S4 input stages of the advanced keys, pending mod-tap resolution
 *   keys_scan_pass_pairs.c     S8 two-key resolvers (deeper key wins, SOCD)
 *   keys_scan_pass_reports.c   S9..S11, S13 gamepad / keyboard / extra / key-state reports
 *
 * Functions declared here are steps that live in another file than their caller.
 */
#ifndef KEYS_KEYS_SCAN_PASS_INTERNAL_H_
#define KEYS_KEYS_SCAN_PASS_INTERNAL_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "keys/keys_types.h"
#include "keys/keys_data.h"
#include "analog/analog_types.h"
#include "analog/analog_data.h"
#include "layout/layout_types.h"
#include "layout/layout_data.h"
#include "tables/layout_keycode_tables.h"

/* ---- Shared pass state ----------------------------------------------------------------------- */

/* Locals of keys_process_scan_pass that more than one step uses. */
typedef struct keys_scan_pass {
    keys_pass_ctx_t ctx;         /* output context, cleared in S0 */
    uint32_t deepest_travel;     /* deepest travel of all events of this pass (RGB signal 1) */
    uint32_t dks_bottom_travel;  /* bottom-out point of the dynamic keystroke stage; zeroed once per pass, see keys_dks_input */
} keys_scan_pass_t;

/* First argument of hid_keystate_report_add_key for an advanced key: action kind 5 in the low
 * byte (stored as HID_KEY_STATE_KIND_ADVANCED), enum keys_advanced_key_kind in the high byte
 * (0x0405 / 0x0505 for the two-key bindings). */
#define KEYS_PASS_KEYSTATE_ADVANCED(kind)  ((uint16_t)(((uint32_t)(kind) << 8) | 5u))

/* ---- Small helpers ----------------------------------------------------------------------------- */

/* Entry kind: only the low byte of the oneof tag is compared. */
static inline uint8_t keys_pass_ak_kind(const AdvancedKey *entry)
{
    return (uint8_t)entry->which_kind;
}

/* Action word of a key code; "no key" (0xFF) is looked up as code 0. */
static inline uint16_t keys_pass_keycode_action(uint32_t keycode)
{
    return k_layout_keycode_actions[keycode == LAYOUT_KEYCODE_NONE ? 0u : keycode];
}

/* Action word of a key on a keymap layer; layers above the highest layer of the profile have no
 * key codes. */
static inline uint16_t keys_pass_layer_action(const analog_key_state_t *key, uint32_t layer)
{
    uint32_t keycode = LAYOUT_KEYCODE_NONE;

    if ((g_layout_layer_state & LAYOUT_STATE_TOP_LAYER_MASK) >= layer)
        keycode = key->keycode[layer];
    return keys_pass_keycode_action(keycode);
}

/* Key record of a key id, NULL if the id is out of range or the position carries no key. */
static inline analog_key_state_t *keys_pass_key_record(uint32_t key_id)
{
    uint32_t col = KEYS_KEY_ID_COL(key_id);

    if (key_id <= KEYS_KEY_ID_MAX && col <= LAYOUT_COLS - 1u) {
        uint8_t scan_pos = g_layout_keyid_to_scanpos[LAYOUT_COLS * (key_id >> 5) + col];

        if (scan_pos != LAYOUT_NO_KEY)
            return &g_analog_key_states[scan_pos];
    }
    return NULL;
}

/* ---- Steps in other files --------------------------------------------------------------------- */

/* keys_scan_pass_advanced.c */
void keys_pass_advanced_key_input(keys_scan_pass_t *pass, keys_key_ref_t *ref, uint32_t travel, uint32_t ak_index); /* S4 */
void keys_pass_modtap_resolve_pending(keys_scan_pass_t *pass, keys_key_ref_t *ref); /* part of S5 */

/* keys_scan_pass_pairs.c */
void keys_pass_resolve_two_key_pairs(keys_scan_pass_t *pass); /* S8 */

/* keys_scan_pass_reports.c */
void keys_pass_send_gamepad_report(keys_scan_pass_t *pass); /* S9 */
bool keys_pass_send_keyboard_report(void); /* S10 */
void keys_pass_send_extra_report(void); /* S11 */
void keys_pass_send_key_state_report(void); /* S13 */

#endif /* KEYS_KEYS_SCAN_PASS_INTERNAL_H_ */
