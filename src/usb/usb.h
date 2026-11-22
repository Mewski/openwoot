/* Public functions of the usb module: TinyUSB callbacks, XInput class driver, suspend state. */
#ifndef USB_USB_H_
#define USB_USB_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "tusb.h"
#include "device/usbd_pvt.h"

#include "usb/usb_types.h"
#include "usb/usb_data.h"

void usb_xinput_init(void);
const usbd_class_driver_t *usbd_app_driver_get_cb(uint8_t *driver_count);
void tud_hid_report_failed_cb(void);
void usb_xinput_reset(uint8_t rhport);
bool usb_xinput_control_xfer_cb(uint8_t rhport, uint8_t stage, const tusb_control_request_t *request);
void tud_umount_cb(void);
void tud_mount_cb(void);
bool usb_xinput_xfer_cb(uint8_t rhport, uint8_t ep_addr, xfer_result_t result, uint32_t xferred_bytes);
bool tud_vendor_control_xfer_cb(uint8_t rhport, uint8_t stage, const tusb_control_request_t *request);
uint16_t usb_xinput_open(uint8_t rhport, const tusb_desc_interface_t *itf_desc, uint16_t max_len);
void tud_resume_cb(void);
void usb_set_suspended(uint32_t suspended);
void tud_suspend_cb(bool remote_wakeup_en);
void usb_resume_if_suspended(void);
const uint8_t *tud_descriptor_configuration_cb(uint8_t index);

/* usb_init.c: start-up steps, each registered in its slot of the start-up list */
void usb_speed_option_init(void);
void usb_serial_string_init(void);
void usb_descriptors_init(void);
void usb_stack_init(void);

/* Further application callbacks are declared by the stack itself (device/usbd.h,
 * class/hid/hid_device.h) and defined in usb_descriptors.c / usb_callbacks.c:
 *   tud_descriptor_device_cb, tud_descriptor_string_cb, tud_hid_descriptor_report_cb,
 *   tud_hid_get_report_cb (returns 0), tud_hid_set_protocol_cb */

/* Pointer tables defined by this unit. */
extern const usbd_class_driver_t k_usb_app_class_driver;                /* usb_xinput.c */
extern const uint8_t *const k_usb_hid_report_desc_itf0_by_mode[3];      /* usb_descriptors.c */

#endif /* USB_USB_H_ */
