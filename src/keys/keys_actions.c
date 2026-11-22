/*
 * keys_actions.c - the single sink for resolved key actions.
 *
 * keys_process_scan_pass (plain keys) and keys_process_advanced_keys (advanced-key bindings)
 * hand every action word of k_layout_keycode_actions to keys_apply_key_action, which logs it
 * in the key-state report and fills the keyboard report (6KRO or NKRO), the system / consumer
 * / mouse-button report, the special-key requests of the pass or the gamepad input.
 */
#include "keys/keys.h"
#include "hid/hid.h"
#include "hid/hid_data.h"
#include "layout/layout.h"
#include "core/run_state.h"
#include "usb/usb_types.h"
#include "usb/usb_data.h"

#define KEYS_USAGE_MODIFIER_FIRST    0xE0u   /* keyboard usage of modifier bit 0 (Left Control) */
#define KEYS_USAGE_MODIFIER_NONE     0xE8u   /* logged for an empty modifier mask */
#define KEYS_MODIFIER_BITS           8u

#define KEYS_NKRO_USAGE_FIRST        0x04u   /* first usage with a bit in the NKRO bitmap */
#define KEYS_NKRO_USAGE_SPLIT        0x33u   /* usages from here on start at report byte 7 */
#define KEYS_NKRO_SPLIT_BYTE         6u      /* ... which is bitmap[6] */

#define KEYS_EXTRA_SYSTEM_FIRST      1u      /* extra codes 1..3: system control */
#define KEYS_EXTRA_SYSTEM_LAST       3u
#define KEYS_EXTRA_BUTTON_FIRST      4u      /* extra codes 4..11: button bits 0..7 */
#define KEYS_EXTRA_BUTTON_LAST       11u

#define KEYS_KEY_STATE_TRAVEL_MAX    1023u   /* 10-bit travel of a key-state entry */
#define KEYS_TRAVEL_MAX              4095u

/* Appends the action to the key-state report (HID interface 4); dropped when 16 entries are
 * stored. Same rules as hid_keystate_report_add_key. */
static void keys_log_key_state(uint16_t action, uint8_t key_id, uint16_t analog, uint8_t pressed)
{
    uint8_t count = g_hid_key_state_report_count;
    uint8_t code = (uint8_t)(action >> LAYOUT_ACTION_CODE_SHIFT);
    uint8_t kind = action & LAYOUT_ACTION_KIND_MASK;
    hid_key_state_entry_t *entry;

    if (count > HID_KEY_STATE_ENTRIES - 1u) {
        return;
    }
    entry = &g_hid_key_state_report.entries[count];

    if (code == 0 && kind == LAYOUT_ACTION_KEYBOARD) {
        /* A key without binding is logged like a special key with code 0. */
        kind = LAYOUT_ACTION_SPECIAL;
    }

    if (kind == LAYOUT_ACTION_MODIFIER) {
        /* Modifier masks are logged as the keyboard usage of their lowest bit. */
        uint8_t usage = KEYS_USAGE_MODIFIER_NONE;

        entry->kind = HID_KEY_STATE_KIND_KEYBOARD;
        for (uint8_t bit = 0; bit != KEYS_MODIFIER_BITS; bit++) {
            if ((code & (1u << bit)) != 0) {
                usage = (uint8_t)(KEYS_USAGE_MODIFIER_FIRST + bit);
                break;
            }
        }
        entry->code = usage;
    } else {
        entry->code = code;
        /* Kinds 2, 3, 4 become HID_KEY_STATE_KIND_CONSUMER, _FN, _GAMEPAD. */
        entry->kind = (kind != LAYOUT_ACTION_KEYBOARD) ? kind + 1u : HID_KEY_STATE_KIND_KEYBOARD;
    }

    entry->key_id = key_id;
    g_hid_key_state_report_count = count + 1u;
    entry->pressed = pressed & 1u;
    entry->travel = KEYS_KEY_STATE_TRAVEL_MAX * (uint32_t)analog / KEYS_TRAVEL_MAX;
}

/* 6KRO / boot keyboard report: modifier mask, or the usage into the first free slot.
 * Duplicates and a seventh key are dropped. */
static void keys_boot_report_add(uint8_t kind, uint8_t code)
{
    if (kind == LAYOUT_ACTION_MODIFIER) {
        g_hid_boot_kbd_report.modifiers |= code;
        return;
    }

    for (uint32_t slot = 0; slot != HID_BOOT_KBD_KEYS; slot++) {
        if (g_hid_boot_kbd_report.keys[slot] == 0) {
            g_hid_boot_kbd_report.keys[slot] = code;
            return;
        }
        if (g_hid_boot_kbd_report.keys[slot] == code) {
            return;
        }
    }
}

/* NKRO keyboard report: modifier mask, or the bit of the usage. Usages 0..3 have no bit. */
static void keys_nkro_report_add(uint8_t kind, uint8_t code)
{
    uint8_t offset;

    if (kind == LAYOUT_ACTION_MODIFIER) {
        g_hid_nkro_report.modifiers |= code;
        return;
    }
    if (code < KEYS_NKRO_USAGE_FIRST) {
        return;
    }

    if (code >= KEYS_NKRO_USAGE_SPLIT) {
        offset = (uint8_t)(code - KEYS_NKRO_USAGE_SPLIT);
        g_hid_nkro_report.bitmap[KEYS_NKRO_SPLIT_BYTE + (offset >> 3)] |= (uint8_t)(1u << (offset & 7u));
    } else {
        offset = (uint8_t)(code - KEYS_NKRO_USAGE_FIRST);
        g_hid_nkro_report.bitmap[offset >> 3] |= (uint8_t)(1u << (offset & 7u));
    }
}

/* System control, mouse-button bits or consumer usage in the report of HID interface 3.
 * System and button reports are only written while no consumer usage is queued; a consumer
 * usage overwrites the report id. */
static void keys_extra_report_add(uint8_t code)
{
    if (code >= KEYS_EXTRA_SYSTEM_FIRST && code <= KEYS_EXTRA_SYSTEM_LAST) {
        if (g_hid_consumer_usage_count == 0) {
            g_hid_extra_report.report_id = HID_EXTRA_REPORT_SYSTEM;
            g_hid_extra_report.system_code = code;
        }
    } else if (code >= KEYS_EXTRA_BUTTON_FIRST && code <= KEYS_EXTRA_BUTTON_LAST) {
        if (g_hid_consumer_usage_count == 0) {
            g_hid_extra_report.report_id = HID_EXTRA_REPORT_BUTTONS;
            g_hid_extra_report.buttons |= (uint8_t)(1u << (code - KEYS_EXTRA_BUTTON_FIRST));
        }
    } else {
        uint8_t usage_count = g_hid_consumer_usage_count;
        uint8_t high_byte;

        if (usage_count > HID_EXTRA_CONSUMER_USAGES - 1u) {
            return;
        }
        /* 16-bit usage: low byte = code, high byte chosen by the code range. */
        g_hid_extra_report.data[2u * usage_count] = code;
        g_hid_extra_report.report_id = HID_EXTRA_REPORT_CONSUMER;
        if (code >= 0x83u && code <= 0x94u) {
            high_byte = 1;
        } else if ((code >= 0x21u && code <= 0x2Au) || code == 0xA2u || code == 0x9Fu) {
            high_byte = 2;
        } else {
            high_byte = 0;
        }
        g_hid_extra_report.data[2u * usage_count + 1u] = high_byte;
        g_hid_consumer_usage_count = usage_count + 1u;
    }
}

/* Special-key (Fn) action: records the request in the pass context and runs the handler on the
 * press edge. Releases are dispatched by keys_process_scan_pass at the end of the pass.
 * Not range-checked: `code` must be below KEYS_FN_ACTION_COUNT. */
static void keys_request_special_action(keys_pass_ctx_t *ctx, uint8_t code, uint8_t key_id, uint8_t layer)
{
    /* The action words of k_layout_keycode_actions only carry codes 1..14; both arrays are
     * indexed with the unchecked code. Matches stock firmware 2.14.1. */
    keys_fn_action_state_t *request = &ctx->fn[code];

    request->key_id = key_id;
    request->layer = layer & KEYS_FN_ACTION_LAYER_MASK;
    request->pressed = 1;

    if (g_keys_fn_action_states[code].pressed) {
        return;
    }
    g_keys_fn_action_states[code] = *request;
    if (code <= KEYS_FN_ACTION_COUNT - 1u) {
        /* Slot 0 of the table is NULL and called unchecked; no key code maps to it. */
        k_layout_key_action_handlers[code](KEYS_KEY_ID_ROW(request->key_id), KEYS_KEY_ID_COL(request->key_id),
                                           request->layer, 1);
    }
}

/* Applies one resolved action word for the key `key_id` on `layer`: logs it in the key-state
 * report, then routes it by kind to the keyboard report (6KRO or NKRO, depending on the active
 * report mode), the system / consumer / button report, the special-key requests of the pass or
 * the gamepad input. Does nothing while the host does not take key reports. A released key
 * (`pressed` == 0) is only logged, unless the action carries LAYOUT_ACTION_ANALOG. `analog` is
 * the 12-bit travel; `force_keyboard` lets a keyboard action through in the gamepad USB modes
 * even when the profile mutes plain keys there. */
void keys_apply_key_action(keys_pass_ctx_t *ctx, uint16_t action, uint8_t key_id, uint8_t layer, uint16_t analog,
                           uint8_t force_keyboard, uint8_t pressed)
{
    uint8_t code = (uint8_t)(action >> LAYOUT_ACTION_CODE_SHIFT);
    uint8_t kind = action & LAYOUT_ACTION_KIND_MASK;

    if ((g_usb_link_flags & USB_LINK_KEY_REPORTS) == 0) {
        return;
    }

    keys_log_key_state(action, key_id, analog, pressed);

    if (pressed == 0 && (action & LAYOUT_ACTION_ANALOG) == 0) {
        return;
    }

    switch (kind) {
    case LAYOUT_ACTION_KEYBOARD:
    case LAYOUT_ACTION_MODIFIER:
        /* In the gamepad USB modes plain keys can be muted by the profile. */
        if ((g_usb_link_flags & USB_LINK_MODE_MASK) != USB_MODE_NO_GAMEPAD && !g_hid_kbd_enabled_in_gamepad_mode
            && force_keyboard == 0) {
            return;
        }
        if ((g_run_state_flags & RUN_STATE_FLAG_6KRO_ACTIVE) != 0) {
            keys_boot_report_add(kind, code);
        } else {
            keys_nkro_report_add(kind, code);
        }
        break;

    case LAYOUT_ACTION_EXTRA:
        keys_extra_report_add(code);
        break;

    case LAYOUT_ACTION_SPECIAL:
        keys_request_special_action(ctx, code, key_id, layer);
        break;

    case LAYOUT_ACTION_GAMEPAD:
        hid_gamepad_apply_binding(&ctx->gamepad, code, analog, 1);
        break;

    default:
        break;
    }
}
