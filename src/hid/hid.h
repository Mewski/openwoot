/* Public functions of the hid module: HID / XInput report building and sending. */
#ifndef HID_HID_H_
#define HID_HID_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "hid/hid_types.h"
#include "hid/hid_data.h"

void hid_update_6kro_active_flag(void);
void hid_keystate_report_add_key(uint16_t keycode, uint32_t travel, uint8_t key_id, uint32_t pressed);
void hid_gamepad_apply_binding(hid_gamepad_input_t *state, uint32_t code, uint32_t travel, uint32_t pressed);
float hid_gamepad_normalize_travel(uint32_t travel);
float hid_gamepad_combine_axis(uint32_t travel_pos, uint32_t travel_neg, uint8_t *priority_state);
float hid_gamepad_apply_response_curve(float t);
void hid_gamepad_shape_stick(float *x, float *y, uint32_t is_left_stick, float min_magnitude);
void hid_send_xinput_report(const hid_xinput_pad_state_t *pad_state);
void hid_send_gamepad_report(const hid_classic_gamepad_report_t *report);

#endif /* HID_HID_H_ */
