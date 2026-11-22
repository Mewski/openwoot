/*
 * hid_data.c - definitions of the RAM globals of the `hid` module. Variables of the .data
 * section carry their initial value; .bss variables have no initializer.
 */
#include "hid/hid_data.h"

/* ---- .data ------------------------------------------------------------------------------------ */

/* Profile cfg option (profile_apply_cfg): when 0 and the USB mode is not 2,
 * keys_apply_key_action drops keyboard usages and modifiers of plain keys. */
uint8_t g_hid_kbd_enabled_in_gamepad_mode = 0x01;

/* XInput player number 0..3 from the host LED command (usb_xinput_xfer_cb),
 * 0xFF = unknown. Only selects the stick corner of the gamepad detection pattern. */
uint8_t g_hid_xinput_player_index = HID_XINPUT_PLAYER_UNKNOWN;

/* ---- .bss ------------------------------------------------------------------------------------- */

/* Set / cleared by feature commands 45 / 46: the gamepad reports carry a fixed pattern. */
uint8_t g_hid_gamepad_detection_active;

/* 1 between tud_mount_cb and tud_umount_cb: send g_hid_key_state_report on interface 4. */
uint8_t g_hid_key_state_report_enabled;

/* 1 between tud_mount_cb and tud_umount_cb: gates hid_send_xinput_report. */
uint8_t g_hid_xinput_report_enabled;

/* Bool of the gamepad mapping file "gm" (keys_apply_gamepad_map): keys with a
 * gamepad binding do not send their keyboard code on layer 0 in a gamepad USB mode. */
uint8_t g_hid_gamepad_keys_exclusive;

/* RAM copy of profile file "gp" (response curve and stick options); valid while
 * g_hid_gamepad_file_flags bit 1 is set. */
GamepadSettings g_hid_gamepad_settings;

/* HID_GAMEPAD_FILE_* bits. */
uint8_t g_hid_gamepad_file_flags;

/* Number of keys with a gamepad binding in the last applied "gm" table. */
uint8_t g_hid_gamepad_mapped_key_count;

/* Used entries (0..16) of g_hid_key_state_report in the current key pass. */
uint8_t g_hid_key_state_report_count;

/* Key-state report built each key pass (interface 4). */
hid_key_state_report_t g_hid_key_state_report;

/* NKRO keyboard report built each key pass (interface 1). */
hid_nkro_report_t g_hid_nkro_report;

/* 6KRO / boot keyboard report built each key pass (interface 1). */
hid_boot_kbd_report_t g_hid_boot_kbd_report;

/* Consumer usages (0..3) queued in g_hid_extra_report during the current key pass. */
uint8_t g_hid_consumer_usage_count;

/* System / consumer / button report built each key pass (interface 3). */
hid_extra_report_t g_hid_extra_report;

/* Opposing-direction resolver state (enum hid_gamepad_axis_socd) of the four stick axes:
 * [0], [1] first stick, [2], [3] second stick. */
uint8_t g_hid_gamepad_stick_axis_socd[HID_GAMEPAD_STICK_AXES];

/* The same for the two extra bipolar axes of the classic gamepad report. */
uint8_t g_hid_gamepad_extra_axis_socd[HID_GAMEPAD_EXTRA_AXES];

/* Last transmitted copies (change detection). */
hid_nkro_report_t g_hid_nkro_report_sent;
hid_extra_report_t g_hid_extra_report_sent;
hid_boot_kbd_report_t g_hid_boot_kbd_report_sent;

/* Last transmitted key-state report; this copy is the buffer handed to tud_hid_n_report. */
hid_key_state_report_t g_hid_key_state_report_sent;

/* Last XInput IN report; also the transfer buffer of the IN endpoint (hid_send_xinput_report). */
hid_xinput_report_t g_hid_xinput_last_report;

/* Last classic gamepad report accepted by tud_hid_n_report (hid_send_gamepad_report). */
hid_classic_gamepad_report_t g_hid_gamepad_last_report;
