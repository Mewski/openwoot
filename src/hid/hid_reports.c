/*
 * hid_reports.c - report helpers of the `hid` module that talk to the USB stack: keyboard
 * report format flag, key-state report entries, gamepad and XInput report senders.
 */
#include <string.h>

#include "hid/hid.h"
#include "core/run_state.h"
#include "layout/layout_types.h"
#include "usb/usb.h"  /* TinyUSB API (tusb.h, usbd_pvt.h) and the usb globals */

#define HID_KEY_STATE_TRAVEL_MAX   1023u  /* 10-bit travel field of a key-state entry */
#define HID_KEY_TRAVEL_MAX         4095u  /* full key travel */
#define HID_USAGE_MODIFIER_FIRST   0xE0u  /* keyboard usage of modifier bit 0 (left control) */
#define HID_USAGE_MODIFIER_NONE    0xE8u  /* reported for an empty modifier mask */

/* Recomputes RUN_STATE_FLAG_6KRO_ACTIVE (keyboard reports use the 8-byte boot format):
 * set while the keyboard interface is not open or in boot protocol, otherwise a copy of
 * RUN_STATE_FLAG_6KRO (report descriptor selected by the device settings).
 * Called on mount, on unmount and when the host changes the HID protocol
 * (tud_hid_set_protocol_cb). */
void hid_update_6kro_active_flag(void)
{
    uint8_t instance = tud_hid_itf_num_to_instance(USB_ITF_KEYBOARD);
    bool boot_format;

    if (instance != USB_HID_ITF_NONE && tud_hid_n_get_protocol(instance) != HID_PROTOCOL_BOOT) {
        boot_format = (g_run_state_flags & RUN_STATE_FLAG_6KRO) != 0;
    } else {
        boot_format = true;
    }

    g_run_state_flags = (uint8_t)((g_run_state_flags & ~RUN_STATE_FLAG_6KRO_ACTIVE)
                                    | (boot_format ? RUN_STATE_FLAG_6KRO_ACTIVE : 0u));
}

/* Appends one entry to the key-state report of this pass; dropped when 16 are stored.
 * keycode = action word of the key (k_layout_keycode_actions), travel = 0..4095,
 * key_id = matrix position byte, pressed = 0 / 1. */
void hid_keystate_report_add_key(uint16_t keycode, uint32_t travel, uint8_t key_id, uint32_t pressed)
{
    uint8_t count = g_hid_key_state_report_count;

    if (count >= HID_KEY_STATE_ENTRIES) {
        return;
    }

    hid_key_state_entry_t *entry = &g_hid_key_state_report.entries[count];
    uint8_t code = (uint8_t)(keycode >> LAYOUT_ACTION_CODE_SHIFT);
    uint8_t action_kind = (uint8_t)(keycode & LAYOUT_ACTION_KIND_MASK);

    /* Action word "keyboard usage 0" = key without binding: reported like an Fn action.
     * (The LAYOUT_ACTION_ANALOG bit is not part of the test.) */
    if (code == 0 && action_kind == LAYOUT_ACTION_KEYBOARD) {
        action_kind = LAYOUT_ACTION_SPECIAL;
    }

    if (action_kind == LAYOUT_ACTION_MODIFIER) {
        /* code is a modifier mask: report the usage of its lowest set bit. */
        uint8_t usage = HID_USAGE_MODIFIER_NONE;

        entry->kind = HID_KEY_STATE_KIND_KEYBOARD;
        for (uint8_t bit = 0; bit < 8; bit++) {
            if (((code >> bit) & 1u) != 0) {
                usage = (uint8_t)(HID_USAGE_MODIFIER_FIRST + bit);
                break;
            }
        }
        entry->code = usage;
    } else {
        entry->code = code;
        if (action_kind != LAYOUT_ACTION_KEYBOARD) {
            /* 2 -> CONSUMER, 3 -> FN, 4 -> GAMEPAD */
            entry->kind = (uint8_t)(action_kind + 1u) & 0xFu;
        } else {
            entry->kind = HID_KEY_STATE_KIND_KEYBOARD;
        }
    }

    g_hid_key_state_report_count = (uint8_t)(count + 1u);
    entry->key_id = key_id;
    /* unk_bit1 keeps its value (0 after the per-pass clear of the report).
     * KNOWN ISSUE: `pressed` is ORed unmasked into the 16-bit word (only its bits 1..5 are
     * masked off), so its bits 6..15 end up in the travel field. This goes wrong when
     * keys_process_advanced_keys passes a raw state byte instead of 0 / 1. Matches stock
     * firmware 2.14.1. */
    entry->pressed = pressed & 1u;
    entry->travel = (uint16_t)((HID_KEY_STATE_TRAVEL_MAX * travel / HID_KEY_TRAVEL_MAX) | (pressed >> 6));
}

/* Sends the 20-byte XInput input report on the IN endpoint of the XInput class driver (USB
 * mode 0) when it differs from the last one sent and the endpoint is free. */
void hid_send_xinput_report(const hid_xinput_pad_state_t *pad_state)
{
    hid_xinput_report_t report = {
        .type = HID_XINPUT_REPORT_TYPE,
        .length = HID_XINPUT_REPORT_LEN,
        .pad = *pad_state,
        .reserved = { 0 },
    };

    if (memcmp(&report, &g_hid_xinput_last_report, offsetof(hid_xinput_report_t, reserved)) == 0) {
        return;
    }
    if (!tud_ready() || g_usb_xinput_ep_in == 0) {
        return;
    }
    if (usbd_edpt_busy(TUD_OPT_RHPORT, g_usb_xinput_ep_in)) {
        return;
    }

    /* The report is remembered before the endpoint is claimed: if the claim fails it is
     * not sent and not retried until the pad state changes again. */
    g_hid_xinput_last_report = report;
    if (usbd_edpt_claim(TUD_OPT_RHPORT, g_usb_xinput_ep_in)) {
        usbd_edpt_xfer(TUD_OPT_RHPORT, g_usb_xinput_ep_in, (uint8_t *)&g_hid_xinput_last_report,
                       HID_XINPUT_REPORT_SIZE);
    }
}

/* Sends the 25-byte classic gamepad report on HID interface 0 (USB mode 1) when it differs
 * from the last one sent; the last report is updated only when the transfer was started. */
void hid_send_gamepad_report(const hid_classic_gamepad_report_t *report)
{
    if (memcmp(&g_hid_gamepad_last_report, report, HID_GAMEPAD_REPORT_SIZE) == 0) {
        return;
    }
    if (!tud_hid_n_ready(tud_hid_itf_num_to_instance(USB_ITF_GAMEPAD))) {
        return;
    }
    if (tud_hid_n_report(tud_hid_itf_num_to_instance(USB_ITF_GAMEPAD), 0, report, HID_GAMEPAD_REPORT_SIZE)) {
        g_hid_gamepad_last_report = *report;
    }
}
