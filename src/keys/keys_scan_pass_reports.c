/*
 * keys_scan_pass_reports.c - report building and sending at the end of the key pass (steps
 * S9, S10, S11 and S13 of keys_process_scan_pass).
 *
 *   S9   keys_pass_send_gamepad_report    XInput pad state (USB mode 0) or classic 25-byte HID
 *                                         gamepad report (USB mode 1) from the pass context
 *   S10  keys_pass_send_keyboard_report   6KRO or NKRO keyboard report, HID interface 1
 *   S11  keys_pass_send_extra_report      system control / consumer / buttons, HID interface 3
 *   S13  keys_pass_send_key_state_report  64-byte key-state report, HID interface 4
 *
 * The keyboard, extra and key-state reports were filled during the pass by keys_apply_key_action
 * and hid_keystate_report_add_key; they are sent only when they differ from the copy that was
 * transmitted last. The report layouts are the types of hid/hid_types.h.
 */
#include <string.h>

#include "tusb.h"

#include "keys/keys_scan_pass_internal.h"
#include "hid/hid.h"
#include "core/run_state.h"
#include "usb/usb_types.h"
#include "usb/usb_data.h"
#include "tables/hid_tables.h"

/* Top end of the analog trigger range: the response curve output between the two knees is
 * stretched so that TRIGGER_KNEE_FULL maps to 1.0. */
#define TRIGGER_KNEE_LOW     0.965f
#define TRIGGER_KNEE_FULL    0.992f
#define TRIGGER_STRETCH_MUL  1.2962968f
#define TRIGGER_STRETCH_ADD  (-0.2859264f)

#define XINPUT_AXIS_POS      32767.0f
#define XINPUT_AXIS_NEG      32768.0f
#define XINPUT_TRIGGER_MAX   255.0f
#define XINPUT_STICK_MIN     0.00003051851f  /* 1 / 32767: smallest stick magnitude that is kept */
#define GAMEPAD_AXIS_SCALE   16383.0f
#define GAMEPAD_TRIGGER_MAX  32767.0f
#define GAMEPAD_STICK_MIN    0.00006103888f  /* 1 / 16383 */

#define XINPUT_PLAYER_COUNT  4u  /* players with a detection pattern in k_hid_xinput_detection_stick */

/* ---- Shared helpers ----------------------------------------------------------------------------- */

/* Travel of an analog trigger binding -> axis value 0..full_scale: dead zone, response curve,
 * top-end stretch. */
static uint32_t keys_pass_trigger_value(uint16_t travel, float scale, uint32_t full_scale)
{
    float level;

    if (travel == 0)
        return 0;
    level = hid_gamepad_normalize_travel(travel);
    if (level == 0.0f)
        return 0;
    level = hid_gamepad_apply_response_curve(level);
    if (level <= TRIGGER_KNEE_LOW || level >= TRIGGER_KNEE_FULL) {
        if (level < TRIGGER_KNEE_FULL)
            return (uint32_t)(level * scale);
        return full_scale;
    }
    return (uint32_t)((level * TRIGGER_STRETCH_MUL + TRIGGER_STRETCH_ADD) * scale);
}

/* Sends a report on the HID interface with the given interface number if that interface can
 * take one; remembers it as transmitted when the transfer was started. The instance lookup is
 * done twice. */
static void keys_pass_send_changed_report(uint8_t itf_num, const void *report, void *sent_copy, uint16_t len)
{
    if (!tud_hid_n_ready(tud_hid_itf_num_to_instance(itf_num)))
        return;
    if (tud_hid_n_report(tud_hid_itf_num_to_instance(itf_num), 0, report, len))
        memcpy(sent_copy, report, len);
}

/* ---- S9a / S9c: XInput -------------------------------------------------------------------------- */

/* Fixed pattern while the host software runs its gamepad detection: the left stick points into
 * the corner that belongs to the XInput player index, so the host can tell which pad this is. */
static void keys_pass_build_xinput_detection_state(hid_xinput_pad_state_t *pad)
{
    uint8_t player = g_hid_xinput_player_index;

    memset(pad, 0, sizeof(*pad));
    if (player > XINPUT_PLAYER_COUNT - 1u) {
        /* No player index from the host yet. */
        pad->rx = 0x7FFF;
        pad->ry = 0x7FFF;
    } else {
        float x = k_hid_xinput_detection_stick[player];
        float y = k_hid_xinput_detection_stick[XINPUT_PLAYER_COUNT + player];
        float right = k_hid_xinput_detection_stick[2u * XINPUT_PLAYER_COUNT + player];

        pad->lx = (int16_t)(int32_t)(x * (x >= 0.0f ? XINPUT_AXIS_POS : XINPUT_AXIS_NEG));
        pad->ly = (int16_t)(int32_t)(y * (y >= 0.0f ? XINPUT_AXIS_POS : XINPUT_AXIS_NEG));
        pad->rx = (int16_t)(int32_t)(right * XINPUT_AXIS_POS);
        pad->ry = pad->rx;
    }
}

/* One XInput stick from four direction travels. Y is positive = up in XInput, the combined axis
 * is positive = down. */
static void keys_pass_xinput_stick(uint16_t right, uint16_t left, uint16_t down, uint16_t up,
                                   uint8_t *socd_state, uint32_t is_left_stick, int16_t *out_x, int16_t *out_y)
{
    float x = hid_gamepad_combine_axis(right, left, &socd_state[0]);
    float y = hid_gamepad_combine_axis(down, up, &socd_state[1]);

    hid_gamepad_shape_stick(&x, &y, is_left_stick, XINPUT_STICK_MIN);
    *out_x = (int16_t)(int32_t)(x * (x < 0.0f ? XINPUT_AXIS_NEG : XINPUT_AXIS_POS));
    *out_y = (int16_t)(int32_t)(-(y * (y > 0.0f ? XINPUT_AXIS_NEG : XINPUT_AXIS_POS)));
}

/* XInput pad state from the gamepad input of the pass: buttons, both triggers (0..255) and both
 * sticks. All zero when no gamepad binding gave input in this pass. */
static void keys_pass_build_xinput_state(const hid_gamepad_input_t *input, hid_xinput_pad_state_t *pad)
{
    memset(pad, 0, sizeof(*pad));
    if (!input->has_input)
        return;

    pad->buttons = (uint16_t)(input->xinput.buttons_lo | (input->xinput.buttons_hi << 8));
    pad->right_trigger = (uint8_t)keys_pass_trigger_value(input->xinput.right_trigger, XINPUT_TRIGGER_MAX, 0xFF);
    pad->left_trigger = (uint8_t)keys_pass_trigger_value(input->xinput.left_trigger, XINPUT_TRIGGER_MAX, 0xFF);

    keys_pass_xinput_stick(input->xinput.ls_right, input->xinput.ls_left,
                           input->xinput.ls_down, input->xinput.ls_up,
                           &g_hid_gamepad_stick_axis_socd[0], 1, &pad->lx, &pad->ly);
    keys_pass_xinput_stick(input->xinput.rs_right, input->xinput.rs_left,
                           input->xinput.rs_down, input->xinput.rs_up,
                           &g_hid_gamepad_stick_axis_socd[2], 0, &pad->rx, &pad->ry);
}

/* ---- S9b / S9d: classic HID gamepad ------------------------------------------------------------- */

/* Report with nothing pressed: the six bipolar axes centred, everything else 0. */
static void keys_pass_init_gamepad_report(hid_classic_gamepad_report_t *report)
{
    memset(report, 0, sizeof(*report));
    report->x = HID_GAMEPAD_AXIS_CENTER;
    report->y = HID_GAMEPAD_AXIS_CENTER;
    report->rx = HID_GAMEPAD_AXIS_CENTER;
    report->ry = HID_GAMEPAD_AXIS_CENTER;
    report->extra[0] = HID_GAMEPAD_AXIS_CENTER;
    report->extra[1] = HID_GAMEPAD_AXIS_CENTER;
}

/* One of the two extra bipolar axes: the response curve is applied to the magnitude. */
static uint16_t keys_pass_gamepad_extra_axis(uint16_t travel_pos, uint16_t travel_neg, uint8_t *socd_state)
{
    float axis = hid_gamepad_combine_axis(travel_pos, travel_neg, socd_state);
    float magnitude;
    float curved;

    if (axis < 0.0f) {
        magnitude = -axis;
        curved = hid_gamepad_apply_response_curve(magnitude);
    } else if (axis == 0.0f) {
        curved = 0.0f;
    } else {
        curved = hid_gamepad_apply_response_curve(axis);
    }
    if (axis < 0.0f)
        return (uint16_t)((int32_t)(-(curved * GAMEPAD_AXIS_SCALE)) + (int32_t)HID_GAMEPAD_AXIS_CENTER);
    return (uint16_t)((int32_t)(curved * GAMEPAD_AXIS_SCALE) + (int32_t)HID_GAMEPAD_AXIS_CENTER);
}

/* Hat switch value from the four d-pad bits (bit 0 up, 1 down, 2 left, 3 right): 1..8 clockwise
 * from up, 0 = centred. Opposite directions cancel each other; they are also cleared in the
 * pass context itself. */
static uint8_t keys_pass_gamepad_hat(hid_gamepad_input_t *input)
{
    uint8_t dpad = input->dpad;

    if ((dpad & 0x03u) == 0x03u) {
        dpad &= 0xFCu;
        input->dpad = dpad;
    }
    if ((dpad & 0x0Cu) == 0x0Cu) {
        dpad &= 0xF3u;
        input->dpad = dpad;
    }

    if (dpad & 0x01u)                       /* up */
        return (dpad & 0x04u) ? 8 : (dpad & 0x08u) ? 2 : 1;
    if (dpad & 0x02u)                       /* down */
        return (dpad & 0x04u) ? 6 : (dpad & 0x08u) ? 4 : 5;
    if (dpad & 0x04u)                       /* left */
        return 7;
    return (dpad & 0x08u) ? 3 : 0;          /* right / centred */
}

/* Classic HID gamepad report from the gamepad input of the pass: buttons, five unipolar axes,
 * two extra bipolar axes, both sticks and the hat switch. The "nothing pressed" report when no
 * gamepad binding gave input in this pass. */
static void keys_pass_build_gamepad_report(hid_gamepad_input_t *input, hid_classic_gamepad_report_t *report)
{
    float x;
    float y;

    keys_pass_init_gamepad_report(report);
    if (!input->has_input)
        return;

    report->buttons = (uint16_t)(input->buttons_lo | (input->buttons_hi << 8));

    report->unipolar[1] = (uint16_t)keys_pass_trigger_value(input->gamepad.unipolar1, GAMEPAD_TRIGGER_MAX, HID_GAMEPAD_TRIGGER_MAX);
    report->unipolar[0] = (uint16_t)keys_pass_trigger_value(input->gamepad.unipolar0, GAMEPAD_TRIGGER_MAX, HID_GAMEPAD_TRIGGER_MAX);
    report->unipolar[2] = (uint16_t)keys_pass_trigger_value(input->gamepad.unipolar2, GAMEPAD_TRIGGER_MAX, HID_GAMEPAD_TRIGGER_MAX);
    report->unipolar[3] = (uint16_t)keys_pass_trigger_value(input->gamepad.unipolar3, GAMEPAD_TRIGGER_MAX, HID_GAMEPAD_TRIGGER_MAX);
    report->unipolar[4] = (uint16_t)keys_pass_trigger_value(input->gamepad.unipolar4, GAMEPAD_TRIGGER_MAX, HID_GAMEPAD_TRIGGER_MAX);

    report->extra[0] = keys_pass_gamepad_extra_axis(input->gamepad.extra0_pos, input->gamepad.extra0_neg,
                                                    &g_hid_gamepad_extra_axis_socd[0]);
    report->extra[1] = keys_pass_gamepad_extra_axis(input->gamepad.extra1_pos, input->gamepad.extra1_neg,
                                                    &g_hid_gamepad_extra_axis_socd[1]);

    x = hid_gamepad_combine_axis(input->gamepad.x_pos, input->gamepad.x_neg, &g_hid_gamepad_stick_axis_socd[0]);
    y = hid_gamepad_combine_axis(input->gamepad.y_pos, input->gamepad.y_neg, &g_hid_gamepad_stick_axis_socd[1]);
    hid_gamepad_shape_stick(&x, &y, 1, GAMEPAD_STICK_MIN);
    report->x = (uint16_t)((int32_t)(x * GAMEPAD_AXIS_SCALE) + (int32_t)HID_GAMEPAD_AXIS_CENTER);
    report->y = (uint16_t)((int32_t)(y * GAMEPAD_AXIS_SCALE) + (int32_t)HID_GAMEPAD_AXIS_CENTER);

    x = hid_gamepad_combine_axis(input->gamepad.rx_pos, input->gamepad.rx_neg, &g_hid_gamepad_stick_axis_socd[2]);
    y = hid_gamepad_combine_axis(input->gamepad.ry_pos, input->gamepad.ry_neg, &g_hid_gamepad_stick_axis_socd[3]);
    hid_gamepad_shape_stick(&x, &y, 0, GAMEPAD_STICK_MIN);
    report->rx = (uint16_t)((int32_t)(x * GAMEPAD_AXIS_SCALE) + (int32_t)HID_GAMEPAD_AXIS_CENTER);
    report->ry = (uint16_t)((int32_t)(y * GAMEPAD_AXIS_SCALE) + (int32_t)HID_GAMEPAD_AXIS_CENTER);

    report->hat = keys_pass_gamepad_hat(input);
}

/* ---- S9 ----------------------------------------------------------------------------------------- */

/* Builds the report of interface 0 for the current USB mode and hands it to the sender of that
 * mode (both send only when the report changed). Mode 2 / 3: no gamepad interface. */
void keys_pass_send_gamepad_report(keys_scan_pass_t *pass)
{
    uint32_t mode = g_usb_link_flags & USB_LINK_MODE_MASK;

    if (mode == USB_MODE_XINPUT) {
        hid_xinput_pad_state_t pad;

        if (g_hid_gamepad_detection_active)
            keys_pass_build_xinput_detection_state(&pad);          /* S9a */
        else
            keys_pass_build_xinput_state(&pass->ctx.gamepad, &pad); /* S9c */
        if (g_hid_xinput_report_enabled)
            hid_send_xinput_report(&pad);
    } else if (mode == USB_MODE_GAMEPAD) {
        hid_classic_gamepad_report_t report;

        if (g_hid_gamepad_detection_active) {
            /* S9b: fixed pattern, first stick fully right / down. */
            keys_pass_init_gamepad_report(&report);
            report.x = HID_GAMEPAD_DETECT_AXIS;
            report.y = HID_GAMEPAD_DETECT_AXIS;
        } else {
            keys_pass_build_gamepad_report(&pass->ctx.gamepad, &report);  /* S9d */
        }
        hid_send_gamepad_report(&report);
    }
}

/* ---- S10 ---------------------------------------------------------------------------------------- */

/* Keyboard report: 8-byte 6KRO / boot format while g_run_state_flags bit 2 is set, else the
 * 28-byte NKRO bitmap. Returns true when the report had changed and a send was attempted (USB
 * code ran): the caller then tests USB_LINK_KEY_REPORTS again. */
bool keys_pass_send_keyboard_report(void)
{
    if (g_run_state_flags & RUN_STATE_FLAG_6KRO_ACTIVE) {
        if (memcmp(&g_hid_boot_kbd_report_sent, &g_hid_boot_kbd_report, sizeof(g_hid_boot_kbd_report)) == 0)
            return false;
        keys_pass_send_changed_report(USB_ITF_KEYBOARD, &g_hid_boot_kbd_report, &g_hid_boot_kbd_report_sent,
                                      sizeof(g_hid_boot_kbd_report));
    } else {
        if (memcmp(&g_hid_nkro_report_sent, &g_hid_nkro_report, sizeof(g_hid_nkro_report)) == 0)
            return false;
        keys_pass_send_changed_report(USB_ITF_KEYBOARD, &g_hid_nkro_report, &g_hid_nkro_report_sent,
                                      sizeof(g_hid_nkro_report));
    }
    return true;
}

/* ---- S11 ---------------------------------------------------------------------------------------- */

/* Sends the system control / consumer / button report on HID interface 3 when it differs from
 * the copy that was transmitted last. */
void keys_pass_send_extra_report(void)
{
    if (memcmp(&g_hid_extra_report_sent, &g_hid_extra_report, sizeof(g_hid_extra_report)) == 0)
        return;
    keys_pass_send_changed_report(USB_ITF_EXTRA, &g_hid_extra_report, &g_hid_extra_report_sent,
                                  sizeof(g_hid_extra_report));
}

/* ---- S13 ---------------------------------------------------------------------------------------- */

/* Sends the key-state report on HID interface 4 when it is enabled and differs from the copy
 * that was transmitted last. Unlike the other reports the copy is updated before the transfer
 * is started and the result of tud_hid_n_report is ignored; the copy itself is what gets
 * transmitted. */
void keys_pass_send_key_state_report(void)
{
    if (!g_hid_key_state_report_enabled)
        return;
    if (memcmp(&g_hid_key_state_report_sent, &g_hid_key_state_report, sizeof(g_hid_key_state_report)) == 0)
        return;
    if (!tud_hid_n_ready(tud_hid_itf_num_to_instance(USB_ITF_KEY_STATE)))
        return;
    g_hid_key_state_report_sent = g_hid_key_state_report;
    tud_hid_n_report(tud_hid_itf_num_to_instance(USB_ITF_KEY_STATE), 0, &g_hid_key_state_report_sent,
                     sizeof(g_hid_key_state_report_sent));
}
