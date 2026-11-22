/*
 * usb_data.c - definitions of the RAM globals of the `usb` module. Variables of the .data
 * section carry their initial value; .bss variables have no initializer.
 */
#include "usb/usb_data.h"

#include "tables/usb_tables.h"  /* k_usb_langid, k_usb_hid_report_desc_kbd_nkro */

/* ---- .data ------------------------------------------------------------------------------------ */

/* USB mode, host LED state and link state bits (USB_LINK_*). With its zero initializer the
 * compiler places it in .bss. */
uint8_t g_usb_link_flags = 0x00;

/* Polling rate selector of the interface-0 endpoints (enum usb_gamepad_poll_rate), from the
 * device settings; default 1 ms. */
uint8_t g_usb_gamepad_poll_rate = USB_GAMEPAD_POLL_1MS;

/* String descriptor sources by string index. */
usb_string_table_t g_usb_string_table = {
    .by_index = {
        [USB_STRING_LANGID]       = (const char *)k_usb_langid,
        [USB_STRING_MANUFACTURER] = BOARD_USB_MANUFACTURER,
        [USB_STRING_PRODUCT]      = BOARD_USB_PRODUCT,
        [USB_STRING_SERIAL]       = "",
    },
};

/* Device descriptor; usb_descriptors_init patches idProduct and bcdDevice before tud_init. */
usb_device_descriptor_t g_usb_device_descriptor = {
    .bLength            = 18,
    .bDescriptorType    = 0x01,
    .bcdUSB             = 0x0200,
    .bDeviceClass       = 0x00,
    .bDeviceSubClass    = 0x00,
    .bDeviceProtocol    = 0x00,
    .bMaxPacketSize0    = USB_EP0_SIZE,
    .idVendor           = USB_VID,
    .idProduct          = USB_PID_BASE,
    .bcdDevice          = USB_BCD_DEVICE_IMAGE,
    .iManufacturer      = USB_STRING_MANUFACTURER,
    .iProduct           = USB_STRING_PRODUCT,
    .iSerialNumber      = USB_STRING_SERIAL,
    .bNumConfigurations = 1,
};

/* HID report descriptor of the keyboard interface; usb_descriptors_init selects the 6KRO or NKRO one. */
const uint8_t *g_usb_kbd_report_descriptor = k_usb_hid_report_desc_kbd_nkro;

/* Microsoft OS 1.0 string descriptor (index 0xEE); the signature is filled in on request. */
usb_msos_string_descriptor_t g_usb_msos_string_descriptor = {
    .bLength         = 0x12,
    .bDescriptorType = 0x03,
    .qwSignature     = { 0, 0, 0, 0, 0, 0, 0 },
    .bMS_VendorCode  = USB_MSOS_VENDOR_CODE,
    .bPad            = 0x00,
};

/* ---- .bss ------------------------------------------------------------------------------------- */

/* HID endpoint transfers that completed with an error; never read. */
uint32_t g_usb_hid_xfer_error_count;

/* 1 between tud_mount_cb and tud_umount_cb; gates protocol responses and events. */
uint8_t g_usb_mounted;

/* OUT endpoint address of the XInput interface (0x01), 0 = not open. */
uint8_t g_usb_xinput_ep_out;

/* Receive buffer of the XInput OUT endpoint (LED / rumble commands), 4-byte aligned. */
uint8_t g_usb_xinput_out_buf[USB_XINPUT_OUT_BUF_SIZE] __attribute__((aligned(4)));

/* IN endpoint address of the XInput interface (0x81), 0 = not open. */
uint8_t g_usb_xinput_ep_in;

/* Serial number string, formatted by storage_format_serial_string from the device info. */
char g_usb_serial_string_buf[USB_SERIAL_STRING_SIZE];

/* Scratch buffer for string descriptors: header word, then up to 31 UTF-16 code units. */
uint16_t g_usb_desc_str[USB_DESC_STR_UNITS];
