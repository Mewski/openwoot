/*
 * usb_types.h - types and constants of the `usb` module (USB glue around TinyUSB:
 * descriptors, mount / suspend callbacks, XInput class driver).
 *
 * TinyUSB's own state (_usbd_dev, _hidd_itf, _ctrl_xfer, dcd xfer_status, ...) is defined
 * by third_party/tinyusb and is not declared here.
 */
#ifndef USB_TYPES_H
#define USB_TYPES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "board.h"

/* ---- USB identity (g_usb_device_descriptor, patched by usb_descriptors_init) ---------------- */

#define USB_VID                  BOARD_USB_VID
#define USB_PID_BASE             BOARD_USB_PID_BASE  /* idProduct = USB_PID_BASE | usb mode (usb_descriptors_init) */
#define USB_BCD_DEVICE_NKRO      0x0250u  /* bcdDevice when g_run_state_flags bit 1 is clear */
#define USB_BCD_DEVICE_6KRO      0x0251u  /* bcdDevice when g_run_state_flags bit 1 (6KRO) is set */
#define USB_BCD_DEVICE_IMAGE     0x0101u  /* initial value in .data, always overwritten by usb_descriptors_init */

/* ---- g_usb_link_flags ----------------------------------------------------------------------- */

/* USB mode = g_usb_link_flags & USB_LINK_MODE_MASK; selects interface 0 and is added to the PID. */
enum usb_mode {
    USB_MODE_XINPUT     = 0,  /* interface 0 = XInput interface (MS OS descriptors served) */
    USB_MODE_GAMEPAD    = 1,  /* interface 0 = classic HID gamepad, 25-byte report */
    USB_MODE_NO_GAMEPAD = 2,  /* interface 0 = 8-byte vendor-defined HID placeholder; 3 behaves the same */
};

#define USB_LINK_MODE_MASK       0x03u  /* bits 1:0 enum usb_mode (from the device settings, settings_apply_usb_mode) */
#define USB_LINK_CAPS_LOCK       0x04u  /* bit 2 host LED state (tud_hid_set_report_cb) */
#define USB_LINK_NUM_LOCK        0x08u  /* bit 3 */
#define USB_LINK_SCROLL_LOCK     0x10u  /* bit 4 */
#define USB_LINK_KEY_REPORTS     0x20u  /* bit 5 key reports on: tud_mount_cb / command 22 set, tud_umount_cb / command 21 clear */
#define USB_LINK_SUSPENDED       0x40u  /* bit 6 bus suspended (usb_set_suspended) */
#define USB_LINK_CONFIGURED      0x80u  /* bit 7 configured (tud_mount_cb / tud_umount_cb) */

/* g_usb_gamepad_poll_rate & 3: polling rate of the interface-0 interrupt endpoints
 * (tud_descriptor_configuration_cb). */
enum usb_gamepad_poll_rate {
    USB_GAMEPAD_POLL_4MS   = 0,  /* bInterval 4 (full speed) / 6 (high speed) */
    USB_GAMEPAD_POLL_1MS   = 1,  /* bInterval 1 (full speed) / 4 (high speed); 3 behaves the same */
    USB_GAMEPAD_POLL_125US = 2,  /* bInterval 1 on a high-speed link, 1 ms on full speed */
};
#define USB_GAMEPAD_POLL_RATE_MASK  0x03u

/* ---- Interfaces and endpoints (configuration descriptors) ----------------------------------- */

enum usb_itf_num {
    USB_ITF_GAMEPAD   = 0,  /* XInput interface, HID gamepad or placeholder (by usb mode) */
    USB_ITF_KEYBOARD  = 1,  /* boot keyboard: 6KRO 8 bytes or NKRO 28 bytes; LED output report */
    USB_ITF_PROTOCOL  = 2,  /* host protocol (Wootility): feature reports, OUT 0x03, IN 0x83 */
    USB_ITF_EXTRA     = 3,  /* system control / consumer / buttons, 7-byte report */
    USB_ITF_KEY_STATE = 4,  /* 64-byte vendor-defined input report (key states) */
};

#define USB_HID_ITF_COUNT        6u       /* CFG_TUD_HID: elements of TinyUSB _hidd_itf */
#define USB_HID_ITF_NONE         0xFFu    /* instance value when no open interface has the wanted number */
#define USB_HID_EP_BUFSIZE       0x800u   /* CFG_TUD_HID_EP_BUFSIZE */
#define USB_EP0_SIZE             64u      /* CFG_TUD_ENDPOINT0_SIZE */
#define USB_RHPORT_INVALID       0xFFu    /* TinyUSB _usbd_rhport before tud_init */

/* ---- XInput class driver (usb_xinput_open, usb_xinput_xfer_cb) ------------------------------- */

#define USB_XINPUT_ITF_CLASS       0xFFu
#define USB_XINPUT_ITF_SUBCLASS    0x5Du
#define USB_XINPUT_DRIVER_LEN      40u    /* bytes of the configuration descriptor consumed by open */
#define USB_XINPUT_OUT_BUF_SIZE    64u    /* g_usb_xinput_out_buf, length armed on the OUT endpoint */
#define USB_XINPUT_OUT_TYPE_LED    0x01u  /* out_buf[0] of the LED command */
#define USB_XINPUT_OUT_LED_LEN     0x03u  /* out_buf[1] of the LED command; out_buf[2] = pattern */
/* LED pattern -> player index: 2 or 6 -> 0, 3 or 7 -> 1, 4 or 8 -> 2, 5 or 9 -> 3, else 0xFF */

/* ---- Microsoft OS 1.0 descriptors -------------------------------------------------------------- */

#define USB_MSOS_STRING_INDEX      0xEEu  /* string descriptor index */
#define USB_MSOS_VENDOR_CODE       0xB0u  /* request code announced in the OS string / bRequest (tud_vendor_control_xfer_cb) */
#define USB_MSOS_COMPAT_ID_INDEX   4u     /* wIndex of the Extended Compat ID request */

/* "MSFT100" OS string descriptor, 18 bytes in .data. The signature characters are zero in the
 * initial value and written on every request. */
typedef struct usb_msos_string_descriptor {
    uint8_t  bLength;          /* +0x00 0x12 */
    uint8_t  bDescriptorType;  /* +0x01 0x03 (string) */
    uint16_t qwSignature[7];   /* +0x02 "MSFT100" in UTF-16LE */
    uint8_t  bMS_VendorCode;   /* +0x10 0xB0 */
    uint8_t  bPad;             /* +0x11 0 */
} usb_msos_string_descriptor_t;

_Static_assert(sizeof(usb_msos_string_descriptor_t) == 18, "usb_msos_string_descriptor_t size");
_Static_assert(offsetof(usb_msos_string_descriptor_t, qwSignature) == 2, "usb_msos_string_descriptor_t.qwSignature");
_Static_assert(offsetof(usb_msos_string_descriptor_t, bMS_VendorCode) == 16, "usb_msos_string_descriptor_t.bMS_VendorCode");

/* ---- Device descriptor ----------------------------------------------------------------------- */

/* Standard USB device descriptor, 18 bytes (same layout as TinyUSB tusb_desc_device_t).
 * Lives in .data because usb_descriptors_init patches idProduct (+0x0A) and bcdDevice (+0x0C). */
typedef struct __attribute__((packed)) usb_device_descriptor {
    uint8_t  bLength;             /* +0x00 18 */
    uint8_t  bDescriptorType;     /* +0x01 1 */
    uint16_t bcdUSB;              /* +0x02 0x0200 */
    uint8_t  bDeviceClass;        /* +0x04 */
    uint8_t  bDeviceSubClass;     /* +0x05 */
    uint8_t  bDeviceProtocol;     /* +0x06 */
    uint8_t  bMaxPacketSize0;     /* +0x07 64 */
    uint16_t idVendor;            /* +0x08 USB_VID */
    uint16_t idProduct;           /* +0x0A USB_PID_BASE | usb mode (usb_descriptors_init) */
    uint16_t bcdDevice;           /* +0x0C 0x0250 / 0x0251 (usb_descriptors_init) */
    uint8_t  iManufacturer;       /* +0x0E 1 */
    uint8_t  iProduct;            /* +0x0F 2 */
    uint8_t  iSerialNumber;       /* +0x10 3 */
    uint8_t  bNumConfigurations;  /* +0x11 1 */
} usb_device_descriptor_t;

_Static_assert(sizeof(usb_device_descriptor_t) == 18, "usb_device_descriptor_t size");
_Static_assert(offsetof(usb_device_descriptor_t, idProduct) == 0x0A, "usb_device_descriptor_t.idProduct");
_Static_assert(offsetof(usb_device_descriptor_t, bcdDevice) == 0x0C, "usb_device_descriptor_t.bcdDevice");

/* ---- String descriptors ------------------------------------------------------------------------ */

enum usb_string_index {
    USB_STRING_LANGID       = 0,
    USB_STRING_MANUFACTURER = 1,
    USB_STRING_PRODUCT      = 2,
    USB_STRING_SERIAL       = 3,
    USB_STRING_COUNT        = 4,  /* indices above 3 stall */
};

/* String descriptor sources, indexed by the string index of GET_DESCRIPTOR(STRING) (the
 * string_desc_arr of the TinyUSB examples). usb_descriptors_init and usb_serial_string_init
 * rewrite .product and .serial. */
typedef union usb_string_table {
    const char *by_index[USB_STRING_COUNT];
    struct {
        const char *langid;        /* +0x00 -> k_usb_langid (two bytes 09 04, not a C string) */
        const char *manufacturer;  /* +0x04 BOARD_USB_MANUFACTURER */
        const char *product;       /* +0x08 BOARD_USB_PRODUCT; the edition's product string for a unit of the special edition */
        const char *serial;        /* +0x0C "" until usb_serial_string_init points it at g_usb_serial_string_buf */
    };
} usb_string_table_t;

_Static_assert(sizeof(usb_string_table_t) == 16, "usb_string_table_t size");
_Static_assert(offsetof(usb_string_table_t, manufacturer) == 0x04, "usb_string_table_t.manufacturer");
_Static_assert(offsetof(usb_string_table_t, product) == 0x08, "usb_string_table_t.product");
_Static_assert(offsetof(usb_string_table_t, serial) == 0x0C, "usb_string_table_t.serial");

#define USB_SERIAL_STRING_SIZE   35u  /* g_usb_serial_string_buf: memset / format size in usb_serial_string_init */
#define USB_DESC_STR_UNITS       32u  /* g_usb_desc_str: header word + up to 31 UTF-16 code units */
#define USB_DESC_STR_MAX_CHARS   31u

#endif /* USB_TYPES_H */
