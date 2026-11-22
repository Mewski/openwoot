/*
 * usb_data.h - RAM globals of the `usb` module. Each global is described at its definition
 * in usb_data.c. Objects that TinyUSB itself defines (usbd.c, usbd_control.c, hid_device.c,
 * dcd_dwc2.c) are not declared here.
 */
#ifndef USB_DATA_H
#define USB_DATA_H

#include "usb/usb_types.h"

/* .data */
extern uint8_t g_usb_link_flags;
extern uint8_t g_usb_gamepad_poll_rate;
extern usb_string_table_t g_usb_string_table;
extern usb_device_descriptor_t g_usb_device_descriptor;
extern const uint8_t *g_usb_kbd_report_descriptor;
extern usb_msos_string_descriptor_t g_usb_msos_string_descriptor;

/* .bss */
extern uint32_t g_usb_hid_xfer_error_count;
extern uint8_t g_usb_mounted;
extern uint8_t g_usb_xinput_ep_out;
extern uint8_t g_usb_xinput_out_buf[USB_XINPUT_OUT_BUF_SIZE];
extern uint8_t g_usb_xinput_ep_in;
extern char g_usb_serial_string_buf[USB_SERIAL_STRING_SIZE];
extern uint16_t g_usb_desc_str[USB_DESC_STR_UNITS];

#endif /* USB_DATA_H */
