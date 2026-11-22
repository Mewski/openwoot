/*
 * usb_xinput.c - XInput (Xbox 360 wired controller) interface: the TinyUSB application
 * class driver that owns interface 0 in USB mode 0, and the Microsoft OS 1.0 compat-id
 * request that makes Windows load the XUSB driver for it.
 *
 * IN reports are sent by hid_send_xinput_report (hid/hid_reports.c); the only OUT command
 * that is evaluated is the LED pattern, which tells the player number.
 */
#include "usb/usb.h"
#include "hid/hid_data.h"
#include "tables/usb_tables.h"

/* Class driver init hook: nothing to initialise. */
void usb_xinput_init(void)
{
}

/* Class driver bus-reset hook: nothing to reset. */
void usb_xinput_reset(uint8_t rhport)
{
}

/* Claims the interface with class 0xFF / subclass 0x5D: skips the interface descriptor and
 * the class-specific descriptor behind it, opens the interrupt endpoints and arms the OUT
 * endpoint. Returns the number of descriptor bytes consumed (40), 0 if not accepted. */
uint16_t usb_xinput_open(uint8_t rhport, const tusb_desc_interface_t *itf_desc, uint16_t max_len)
{
    TU_VERIFY(itf_desc->bInterfaceClass == USB_XINPUT_ITF_CLASS
              && itf_desc->bInterfaceSubClass == USB_XINPUT_ITF_SUBCLASS, 0);
    TU_VERIFY(max_len >= USB_XINPUT_DRIVER_LEN, 0);

    const uint8_t *p_desc = tu_desc_next(itf_desc);  /* class-specific (0x21) descriptor */
    p_desc = tu_desc_next(p_desc);                   /* first endpoint descriptor */

    TU_ASSERT(usbd_open_edpt_pair(rhport, p_desc, itf_desc->bNumEndpoints, TUSB_XFER_INTERRUPT,
                                  &g_usb_xinput_ep_out, &g_usb_xinput_ep_in), 0);

    if (g_usb_xinput_ep_out != 0) {
        if (!usbd_edpt_xfer(rhport, g_usb_xinput_ep_out, g_usb_xinput_out_buf, USB_XINPUT_OUT_BUF_SIZE)) {
            /* Only a breakpoint: the interface is accepted nevertheless. */
            TU_BREAKPOINT();
        }
    }
    return USB_XINPUT_DRIVER_LEN;
}

/* No class request is supported: everything addressed to the interface stalls. */
bool usb_xinput_control_xfer_cb(uint8_t rhport, uint8_t stage, const tusb_control_request_t *request)
{
    return false;
}

/* Transfer complete on an endpoint of the interface. Only the OUT endpoint matters: an LED
 * command (01 03 pattern) sets g_hid_xinput_player_index, then the endpoint is armed again. */
bool usb_xinput_xfer_cb(uint8_t rhport, uint8_t ep_addr, xfer_result_t result, uint32_t xferred_bytes)
{
    if (ep_addr != g_usb_xinput_ep_out) {
        return true;
    }

    if (result == XFER_RESULT_SUCCESS && xferred_bytes > 2
        && g_usb_xinput_out_buf[0] == USB_XINPUT_OUT_TYPE_LED
        && g_usb_xinput_out_buf[1] == USB_XINPUT_OUT_LED_LEN) {
        switch (g_usb_xinput_out_buf[2]) {  /* LED pattern */
        case 2:  /* flash, then player 1 on */
        case 6:  /* player 1 on */
            g_hid_xinput_player_index = 0;
            break;
        case 3:
        case 7:
            g_hid_xinput_player_index = 1;
            break;
        case 4:
        case 8:
            g_hid_xinput_player_index = 2;
            break;
        case 5:
        case 9:
            g_hid_xinput_player_index = 3;
            break;
        default:
            g_hid_xinput_player_index = HID_XINPUT_PLAYER_UNKNOWN;
            break;
        }
    }

    TU_ASSERT(usbd_edpt_xfer(rhport, g_usb_xinput_ep_out, g_usb_xinput_out_buf, USB_XINPUT_OUT_BUF_SIZE));
    return true;
}

/* The single application class driver (no name field: CFG_TUSB_DEBUG < 2). */
const usbd_class_driver_t k_usb_app_class_driver = {
    .init            = usb_xinput_init,
    .reset           = usb_xinput_reset,
    .open            = usb_xinput_open,
    .control_xfer_cb = usb_xinput_control_xfer_cb,
    .xfer_cb         = usb_xinput_xfer_cb,
    .sof             = NULL,
};

/* TinyUSB hook, called by tud_init: returns the application class drivers. */
const usbd_class_driver_t *usbd_app_driver_get_cb(uint8_t *driver_count)
{
    *driver_count = 1;
    return &k_usb_app_class_driver;
}

/* Vendor requests on the control endpoint. Only in USB mode 0 (XInput): bRequest 0xB0 (the
 * request code announced in the "MSFT100" string descriptor) with wIndex 4 returns the
 * extended compat ID descriptor (compatible id "XUSB10"). Everything else stalls. */
bool tud_vendor_control_xfer_cb(uint8_t rhport, uint8_t stage, const tusb_control_request_t *request)
{
    if (stage != CONTROL_STAGE_SETUP) {
        return true;
    }
    if ((g_usb_link_flags & USB_LINK_MODE_MASK) != USB_MODE_XINPUT) {
        return false;
    }
    if (request->bmRequestType_bit.type == TUSB_REQ_TYPE_VENDOR
        && request->bRequest == USB_MSOS_VENDOR_CODE
        && request->wIndex == USB_MSOS_COMPAT_ID_INDEX) {
        return tud_control_xfer(rhport, request, (void *)(uintptr_t)k_usb_msos_compat_id_desc,
                                sizeof(k_usb_msos_compat_id_desc));
    }
    return false;
}
