/* Constant tables of module `usb` (declarations); each table is described at its definition in
 * usb_tables.c. */
#ifndef TABLES_USB_TABLES_H
#define TABLES_USB_TABLES_H

#include <stdint.h>

extern const uint8_t k_usb_msos_compat_id_desc[40];
extern const uint8_t k_usb_hid_report_desc_kbd_6kro[59];
extern const uint8_t k_usb_hid_report_desc_kbd_nkro[103];
extern const uint16_t k_usb_langid[1];
extern const uint8_t k_usb_config_desc_nkro_gamepad_int4[141];
extern const uint8_t k_usb_config_desc_6kro_xinput_int4[156];
extern const uint8_t k_usb_config_desc_6kro_placeholder[141];
extern const uint8_t k_usb_config_desc_6kro_xinput_int6[156];
extern const uint8_t k_usb_config_desc_nkro_gamepad_int1[141];
extern const uint8_t k_usb_config_desc_6kro_xinput_int1[156];
extern const uint8_t k_usb_config_desc_6kro_gamepad_int4[141];
extern const uint8_t k_usb_config_desc_6kro_gamepad_int6[141];
extern const uint8_t k_usb_config_desc_nkro_gamepad_int6[141];
extern const uint8_t k_usb_config_desc_6kro_gamepad_int1[141];
extern const uint8_t k_usb_config_desc_nkro_xinput_int4[156];
extern const uint8_t k_usb_config_desc_nkro_placeholder[141];
extern const uint8_t k_usb_config_desc_nkro_xinput_int6[156];
extern const uint8_t k_usb_config_desc_nkro_xinput_int1[156];
extern const uint8_t k_usb_hid_report_desc_vendor64[21];
extern const uint8_t k_usb_hid_report_desc_protocol[99];
extern const uint8_t k_usb_hid_report_desc_system_consumer_mouse[98];
extern const uint8_t k_usb_os_detect_ids[6];
extern const uint8_t k_usb_hid_report_desc_itf0_placeholder[21];
extern const uint8_t k_usb_hid_report_desc_gamepad[122];

#endif /* TABLES_USB_TABLES_H */
