/*
 * usb_descriptors.c - descriptor callbacks of the TinyUSB device stack: device,
 * configuration, string (with the Microsoft OS string) and HID report descriptors.
 *
 * The descriptor bytes are the tables of tables/usb_tables.h.
 * Interfaces: 0 gamepad (XInput interface, HID gamepad or placeholder, by USB mode),
 * 1 keyboard, 2 host protocol, 3 system / consumer / mouse buttons, 4 key states.
 */
#include <string.h>

#include "usb/usb.h"
#include "core/run_state.h"
#include "tables/usb_tables.h"

/* HID report descriptor of interface 0 by USB mode (g_usb_link_flags & 3). Mode 0 has no
 * HID interface 0 (XInput), mode 3 is handled before the lookup. */
const uint8_t *const k_usb_hid_report_desc_itf0_by_mode[3] = {
    [USB_MODE_XINPUT]     = NULL,
    [USB_MODE_GAMEPAD]    = k_usb_hid_report_desc_gamepad,
    [USB_MODE_NO_GAMEPAD] = k_usb_hid_report_desc_itf0_placeholder,
};

/* GET_DESCRIPTOR(DEVICE): the descriptor lives in RAM, usb_descriptors_init patches idProduct / bcdDevice. */
uint8_t const *tud_descriptor_device_cb(void)
{
    return (uint8_t const *)&g_usb_device_descriptor;
}

/* bInterval variants of the interface-0 endpoints in the configuration descriptors. */
typedef enum {
    USB_CFG_INTERVAL_1,  /* full speed 1 ms, high speed 125 us */
    USB_CFG_INTERVAL_4,  /* full speed 4 ms, high speed 1 ms */
    USB_CFG_INTERVAL_6,  /* high speed 4 ms */
} usb_cfg_interval_t;

/* bInterval is in frames (1 ms) at full speed and 2^(n-1) microframes (125 us) at high
 * speed, so the same polling rate needs a different descriptor per bus speed. */
static usb_cfg_interval_t usb_select_gamepad_interval(void)
{
    uint8_t poll_rate = g_usb_gamepad_poll_rate & USB_GAMEPAD_POLL_RATE_MASK;

    if (tud_speed_get() == TUSB_SPEED_HIGH) {
        if (poll_rate == USB_GAMEPAD_POLL_4MS) {
            return USB_CFG_INTERVAL_6;
        }
        if (poll_rate == USB_GAMEPAD_POLL_125US) {
            return USB_CFG_INTERVAL_1;
        }
        return USB_CFG_INTERVAL_4;
    }
    if (poll_rate == USB_GAMEPAD_POLL_4MS) {
        return USB_CFG_INTERVAL_4;
    }
    return USB_CFG_INTERVAL_1;
}

/* GET_DESCRIPTOR(CONFIGURATION): one of 14 complete descriptors, chosen by the keyboard
 * report format (6KRO / NKRO), the USB mode and, for the modes with a gamepad interface,
 * the polling interval. `index` is ignored (one configuration). */
const uint8_t *tud_descriptor_configuration_cb(uint8_t index)
{
    bool kbd_6kro = (g_run_state_flags & RUN_STATE_FLAG_6KRO) != 0;
    uint8_t usb_mode = g_usb_link_flags & USB_LINK_MODE_MASK;

    if (usb_mode == USB_MODE_XINPUT) {
        switch (usb_select_gamepad_interval()) {
        case USB_CFG_INTERVAL_6:
            return kbd_6kro ? k_usb_config_desc_6kro_xinput_int6 : k_usb_config_desc_nkro_xinput_int6;
        case USB_CFG_INTERVAL_1:
            return kbd_6kro ? k_usb_config_desc_6kro_xinput_int1 : k_usb_config_desc_nkro_xinput_int1;
        default:
            return kbd_6kro ? k_usb_config_desc_6kro_xinput_int4 : k_usb_config_desc_nkro_xinput_int4;
        }
    }

    if (usb_mode == USB_MODE_GAMEPAD) {
        switch (usb_select_gamepad_interval()) {
        case USB_CFG_INTERVAL_6:
            return kbd_6kro ? k_usb_config_desc_6kro_gamepad_int6 : k_usb_config_desc_nkro_gamepad_int6;
        case USB_CFG_INTERVAL_1:
            return kbd_6kro ? k_usb_config_desc_6kro_gamepad_int1 : k_usb_config_desc_nkro_gamepad_int1;
        default:
            return kbd_6kro ? k_usb_config_desc_6kro_gamepad_int4 : k_usb_config_desc_nkro_gamepad_int4;
        }
    }

    /* Modes 2 and 3: interface 0 is the 8-byte placeholder with a fixed interval. */
    return kbd_6kro ? k_usb_config_desc_6kro_placeholder : k_usb_config_desc_nkro_placeholder;
}

/* GET_DESCRIPTOR(STRING). Index 0: language id table. Index 0xEE in USB mode 0: Microsoft
 * OS string descriptor "MSFT100" + request code. Indices 1..3: g_usb_string_table entry
 * converted to UTF-16 in g_usb_desc_str (at most 31 characters). Others: NULL (stall).
 * (The host-OS fingerprint taken from this request lives in the stack itself:
 * third_party/tinyusb usbd.c process_get_descriptor.) */
uint16_t const *tud_descriptor_string_cb(uint8_t index, uint16_t langid)
{
    uint8_t chr_count;

    if (index == USB_STRING_LANGID) {
        memcpy(&g_usb_desc_str[1], g_usb_string_table.langid, sizeof(uint16_t));
        chr_count = 1;
    } else {
        if ((g_usb_link_flags & USB_LINK_MODE_MASK) == USB_MODE_XINPUT && index == USB_MSOS_STRING_INDEX) {
            static const char signature[] = "MSFT100";

            for (uint8_t i = 0; i < TU_ARRAY_SIZE(g_usb_msos_string_descriptor.qwSignature); i++) {
                g_usb_msos_string_descriptor.qwSignature[i] = (uint8_t)signature[i];
            }
            return (uint16_t const *)&g_usb_msos_string_descriptor;
        }

        if (index >= USB_STRING_COUNT) {
            return NULL;
        }

        const char *str = g_usb_string_table.by_index[index];

        chr_count = (uint8_t)strlen(str);
        if (chr_count > USB_DESC_STR_MAX_CHARS) {
            chr_count = USB_DESC_STR_MAX_CHARS;
        }
        for (uint8_t i = 0; i < chr_count; i++) {
            g_usb_desc_str[1 + i] = (uint8_t)str[i];
        }
    }

    /* Header: descriptor type and length in bytes (including the header). */
    g_usb_desc_str[0] = (uint16_t)((TUSB_DESC_STRING << 8) | (uint8_t)(2 * chr_count + 2));
    return g_usb_desc_str;
}

/* Interface number of an open HID instance, USB_HID_ITF_NONE if the instance is not open.
 * The stack exports only the reverse lookup, so the interface numbers 0..4 are tried in
 * turn. hidd_control_xfer_cb passes instance = get_index_by_itfnum(wIndex), the first open
 * instance with that number. */
static uint8_t usb_hid_instance_itf_num(uint8_t instance)
{
    for (uint8_t itf_num = USB_ITF_GAMEPAD; itf_num <= USB_ITF_KEY_STATE; itf_num++) {
        if (tud_hid_itf_num_to_instance(itf_num) == instance) {
            return itf_num;
        }
    }
    return USB_HID_ITF_NONE;
}

/* GET_DESCRIPTOR(HID REPORT) of a HID interface; NULL for an unknown interface. */
uint8_t const *tud_hid_descriptor_report_cb(uint8_t instance)
{
    switch (usb_hid_instance_itf_num(instance)) {
    case USB_ITF_GAMEPAD: {
        uint8_t usb_mode = g_usb_link_flags & USB_LINK_MODE_MASK;

        if (usb_mode == 3) {
            /* Mode 3 has no table entry: the keyboard descriptor is returned. Matches
             * stock firmware 2.14.1. */
            return g_usb_kbd_report_descriptor;
        }
        return k_usb_hid_report_desc_itf0_by_mode[usb_mode];
    }
    case USB_ITF_KEYBOARD:
        return g_usb_kbd_report_descriptor;
    case USB_ITF_PROTOCOL:
        return k_usb_hid_report_desc_protocol;
    case USB_ITF_EXTRA:
        return k_usb_hid_report_desc_system_consumer_mouse;
    case USB_ITF_KEY_STATE:
        return k_usb_hid_report_desc_vendor64;
    default:
        return NULL;
    }
}
