/*
 * usb_callbacks.c - state callbacks of the TinyUSB device stack: mount / unmount, bus
 * suspend / resume with the switch between the active and the suspended operating state,
 * and the small HID class callbacks.
 *
 * tud_hid_set_report_cb and tud_hid_report_complete_cb belong to the host protocol
 * (protocol/).
 */
#include "usb/usb.h"
#include "adc_scan/adc_scan_data.h"
#include "core/run_state.h"
#include "hid/hid.h"
#include "profile/profile.h"
#include "rgb/rgb_frame.h"

/* SET_CONFIGURATION accepted: the device is usable by the host. */
void tud_mount_cb(void)
{
    g_usb_mounted = 1;
    if ((g_usb_link_flags & USB_LINK_CONFIGURED) == 0) {
        g_usb_link_flags |= USB_LINK_CONFIGURED;
        rgb_update_output_enable();
    }
    g_usb_link_flags |= USB_LINK_KEY_REPORTS;
    g_hid_key_state_report_enabled = 1;
    g_hid_xinput_report_enabled = 1;
    g_hid_xinput_player_index = HID_XINPUT_PLAYER_UNKNOWN;
    hid_update_6kro_active_flag();
}

/* The device left the configured state (bus reset, unplug, SET_CONFIGURATION 0). */
void tud_umount_cb(void)
{
    g_usb_mounted = 0;
    if ((g_usb_link_flags & USB_LINK_CONFIGURED) != 0) {
        g_usb_link_flags &= (uint8_t)~USB_LINK_CONFIGURED;
        rgb_update_output_enable();
    }
    g_usb_link_flags &= (uint8_t)~USB_LINK_KEY_REPORTS;
    g_hid_xinput_player_index = HID_XINPUT_PLAYER_UNKNOWN;
    g_hid_key_state_report_enabled = 0;
    g_hid_xinput_report_enabled = 0;
    hid_update_6kro_active_flag();
}

/* Switches between the active and the USB-suspended operating state.
 * Suspended: key reports off, hall-sensor scan mode level forced to 0, general settings of
 * profile 0 applied. Active: scan mode level of the profile restored and, if a profile
 * other than {0, 0} is active, a reload of it requested from the main loop. */
void usb_set_suspended(uint32_t suspended)
{
    uint8_t flags = g_usb_link_flags & (uint8_t)~(USB_LINK_SUSPENDED | USB_LINK_KEY_REPORTS);

    if ((suspended & 1u) != 0) {
        flags |= USB_LINK_SUSPENDED;
    } else {
        flags |= USB_LINK_KEY_REPORTS;
    }
    g_usb_link_flags = flags;
    rgb_update_output_enable();

    if (suspended != 0) {
        if ((g_run_state_flags & RUN_STATE_SCAN_MODE_MASK) != 0 && g_adc_scan_mode_requested != 0) {
            g_adc_scan_mode_change_pending = 1;
            g_adc_scan_mode_requested = 0;
        }
        if (g_profile_active_id.index != 0 || g_profile_active_id.ns != 0) {
            profile_load_and_apply_cfg(0);
        }
        return;
    }

    uint8_t scan_mode = g_run_state_flags >> RUN_STATE_SCAN_MODE_SHIFT;

    if (g_adc_scan_mode_requested != scan_mode) {
        g_adc_scan_mode_change_pending = 1;
        g_adc_scan_mode_requested = scan_mode;
    }
    if (g_profile_active_id.index != 0 || g_profile_active_id.ns != 0) {
        g_profile_switch_pending = 1;
        g_profile_switch_target = g_profile_active_id;
    }
}

/* Bus suspended. remote_wakeup_en = the host allowed remote wakeup (a key press may wake
 * it, startup_run_suspended_task). */
void tud_suspend_cb(bool remote_wakeup_en)
{
    g_run_state_flags = (uint8_t)((g_run_state_flags & ~RUN_STATE_FLAG_REMOTE_WAKEUP)
                                    | (remote_wakeup_en ? RUN_STATE_FLAG_REMOTE_WAKEUP : 0u));
    if ((g_usb_link_flags & USB_LINK_SUSPENDED) == 0) {
        usb_set_suspended(1);
    }
}

/* Bus resumed (also called on bus reset / unplug while suspended). */
void tud_resume_cb(void)
{
    if ((g_usb_link_flags & USB_LINK_SUSPENDED) != 0) {
        usb_set_suspended(0);
    }
}

/* Same as tud_resume_cb; not called by this firmware. */
void usb_resume_if_suspended(void)
{
    if ((g_usb_link_flags & USB_LINK_SUSPENDED) != 0) {
        usb_set_suspended(0);
    }
}

/* Local addition to the HID class driver (third_party/tinyusb): a transfer on a HID
 * endpoint failed. The counter is never read. */
void tud_hid_report_failed_cb(void)
{
    ++g_usb_hid_xfer_error_count;
}

/* GET_REPORT is not supported: no data. The class driver then answers a request with a
 * report id and wLength > 1 with the id byte alone and stalls every other one. */
uint16_t tud_hid_get_report_cb(uint8_t instance, uint8_t report_id, hid_report_type_t report_type,
                               uint8_t *buffer, uint16_t reqlen)
{
    return 0;
}

/* SET_PROTOCOL (boot / report) on a HID interface: the keyboard report format may change. */
void tud_hid_set_protocol_cb(uint8_t instance, uint8_t protocol)
{
    hid_update_6kro_active_flag();
}
