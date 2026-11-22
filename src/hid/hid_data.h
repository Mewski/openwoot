/*
 * hid_data.h - RAM globals of the `hid` module. Each global is described at its definition
 * in hid_data.c.
 */
#ifndef HID_DATA_H
#define HID_DATA_H

#include "hid/hid_types.h"
#include "proto/settings.pb.h"

/* .data */
extern uint8_t g_hid_kbd_enabled_in_gamepad_mode;
extern uint8_t g_hid_xinput_player_index;

/* .bss */
extern uint8_t g_hid_gamepad_detection_active;
extern uint8_t g_hid_key_state_report_enabled;
extern uint8_t g_hid_xinput_report_enabled;
extern uint8_t g_hid_gamepad_keys_exclusive;
extern GamepadSettings g_hid_gamepad_settings;
extern uint8_t g_hid_gamepad_file_flags;
extern uint8_t g_hid_gamepad_mapped_key_count;
extern uint8_t g_hid_key_state_report_count;
extern hid_key_state_report_t g_hid_key_state_report;
extern hid_nkro_report_t g_hid_nkro_report;
extern hid_boot_kbd_report_t g_hid_boot_kbd_report;
extern uint8_t g_hid_consumer_usage_count;
extern hid_extra_report_t g_hid_extra_report;
extern uint8_t g_hid_gamepad_stick_axis_socd[HID_GAMEPAD_STICK_AXES];
extern uint8_t g_hid_gamepad_extra_axis_socd[HID_GAMEPAD_EXTRA_AXES];
extern hid_nkro_report_t g_hid_nkro_report_sent;
extern hid_extra_report_t g_hid_extra_report_sent;
extern hid_boot_kbd_report_t g_hid_boot_kbd_report_sent;
extern hid_key_state_report_t g_hid_key_state_report_sent;
extern hid_xinput_report_t g_hid_xinput_last_report;
extern hid_classic_gamepad_report_t g_hid_gamepad_last_report;

/* The nanopb structure of profile file "gp" occupies 15 bytes. */
_Static_assert(sizeof(GamepadSettings) == 15, "g_hid_gamepad_settings size");

#endif /* HID_DATA_H */
