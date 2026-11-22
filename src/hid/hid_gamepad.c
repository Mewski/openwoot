/*
 * hid_gamepad.c - gamepad function of the keys: collecting the per-pass inputs of the keys
 * bound to gamepad controls, and the analog-stick math (travel normalisation, response
 * curve, opposing-direction resolution, stick vector shaping).
 *
 * Stick math per pass (called from keys_process_scan_pass):
 *   t    = hid_gamepad_normalize_travel(travel)
 *   axis = hid_gamepad_combine_axis(travel_pos, travel_neg, &socd_state)
 *   hid_gamepad_shape_stick(&x, &y, is_left_stick, one_output_step)
 */
#include <math.h>

#include "hid/hid.h"
#include "usb/usb_data.h"

#define HID_GAMEPAD_TRAVEL_MAX        4095.0f  /* full key travel */
#define HID_GAMEPAD_CURVE_SCALE       255.0f   /* curve point bytes are x / 255 and y / 255 */
#define HID_GAMEPAD_PERCENT_SCALE     100.0f

/* Curve point indices of GamepadResponseCurve.points. */
enum {
    CURVE_POINT_DEADZONE   = 0,  /* x = start of the input range */
    CURVE_POINT_KNEE_LOW   = 1,
    CURVE_POINT_KNEE_HIGH  = 2,
    CURVE_POINT_SATURATION = 3,  /* x = end of the input range */
};

/* True while profile file "gp" is loaded into g_hid_gamepad_settings. */
static inline bool hid_gamepad_settings_loaded(void)
{
    return (g_hid_gamepad_file_flags & HID_GAMEPAD_FILE_GP_LOADED) != 0;
}

/* Axis direction slot: keeps the deepest travel (0..4095) seen in this pass. */
static inline void hid_gamepad_keep_max_travel(uint16_t *slot, uint32_t travel)
{
    if (travel < *slot) {
        travel = *slot;
    }
    *slot = (uint16_t)travel;
}

/* Button bit: ORs the pressed state of the key into the bit(s) `mask` of a button byte. */
static inline void hid_gamepad_or_button(uint8_t *buttons, uint8_t mask, uint32_t pressed)
{
    if ((pressed & 1u) != 0) {
        *buttons |= mask;
    }
}

/* USB mode 0 (XInput), codes 120..144: merges one key into the XInput part of the accumulator.
 * Returns false for a code that is no XInput control. */
static bool hid_gamepad_apply_xinput_binding(hid_gamepad_input_t *state, uint32_t code,
                                             uint32_t travel, uint32_t pressed)
{
    uint8_t *lo = &state->xinput.buttons_lo;  /* wButtons bits 7:0 */
    uint8_t *hi = &state->xinput.buttons_hi;  /* wButtons bits 15:8 */

    switch (code) {
    case 120: hid_gamepad_or_button(hi, (uint8_t)(HID_XINPUT_BTN_A >> 8), pressed); break;
    case 121: hid_gamepad_or_button(hi, (uint8_t)(HID_XINPUT_BTN_B >> 8), pressed); break;
    case 122: hid_gamepad_or_button(hi, (uint8_t)(HID_XINPUT_BTN_X >> 8), pressed); break;
    case 123: hid_gamepad_or_button(hi, (uint8_t)(HID_XINPUT_BTN_Y >> 8), pressed); break;
    case 124: hid_gamepad_or_button(lo, (uint8_t)HID_XINPUT_BTN_START, pressed); break;
    case 125: hid_gamepad_or_button(lo, (uint8_t)HID_XINPUT_BTN_BACK, pressed); break;
    case 126: hid_gamepad_or_button(lo, (uint8_t)HID_XINPUT_BTN_DPAD_UP, pressed); break;
    case 127: hid_gamepad_or_button(lo, (uint8_t)HID_XINPUT_BTN_DPAD_DOWN, pressed); break;
    case 128: hid_gamepad_or_button(lo, (uint8_t)HID_XINPUT_BTN_DPAD_LEFT, pressed); break;
    case 129: hid_gamepad_or_button(lo, (uint8_t)HID_XINPUT_BTN_DPAD_RIGHT, pressed); break;
    case 130: hid_gamepad_or_button(hi, (uint8_t)(HID_XINPUT_BTN_LB >> 8), pressed); break;
    case 131: hid_gamepad_or_button(hi, (uint8_t)(HID_XINPUT_BTN_RB >> 8), pressed); break;
    case HID_GAMEPAD_CODE_GUIDE:
        hid_gamepad_or_button(hi, (uint8_t)(HID_XINPUT_BTN_GUIDE >> 8), pressed);
        break;
    case 133: hid_gamepad_keep_max_travel(&state->xinput.ls_up, travel); break;
    case 134: hid_gamepad_keep_max_travel(&state->xinput.ls_down, travel); break;
    case 135: hid_gamepad_keep_max_travel(&state->xinput.ls_left, travel); break;
    case 136: hid_gamepad_keep_max_travel(&state->xinput.ls_right, travel); break;
    case 137: hid_gamepad_keep_max_travel(&state->xinput.rs_up, travel); break;
    case 138: hid_gamepad_keep_max_travel(&state->xinput.rs_down, travel); break;
    case 139: hid_gamepad_keep_max_travel(&state->xinput.rs_left, travel); break;
    case 140: hid_gamepad_keep_max_travel(&state->xinput.rs_right, travel); break;
    case HID_GAMEPAD_CODE_L3:
        hid_gamepad_or_button(lo, (uint8_t)HID_XINPUT_BTN_L3, pressed);
        break;
    case HID_GAMEPAD_CODE_R3:
        hid_gamepad_or_button(lo, (uint8_t)HID_XINPUT_BTN_R3, pressed);
        break;
    case HID_GAMEPAD_CODE_TRIGGER_L:
        hid_gamepad_keep_max_travel(&state->xinput.left_trigger, travel);
        break;
    case HID_GAMEPAD_CODE_TRIGGER_R:
        hid_gamepad_keep_max_travel(&state->xinput.right_trigger, travel);
        break;
    default:
        return false;
    }
    return true;
}

/* USB mode 1 (classic HID gamepad), codes 120..155 (132, 141 and 142 are XInput only): merges
 * one key into the classic gamepad part of the accumulator. Returns false for a code that is
 * no control of the classic gamepad. */
static bool hid_gamepad_apply_classic_binding(hid_gamepad_input_t *state, uint32_t code,
                                              uint32_t travel, uint32_t pressed)
{
    switch (code) {
    case 120: case 121: case 122: case 123:
    case 124: case 125: case 126: case 127:  /* buttons 1..8 */
        hid_gamepad_or_button(&state->buttons_lo,
                              (uint8_t)(1u << (code - HID_GAMEPAD_CODE_BUTTON_FIRST)), pressed);
        break;
    case 128: case 129: case 130: case 131:  /* buttons 9..12 */
        hid_gamepad_or_button(&state->buttons_hi,
                              (uint8_t)(1u << (code - HID_GAMEPAD_CODE_BUTTON_FIRST - 8u)), pressed);
        break;
    case 133: hid_gamepad_keep_max_travel(&state->gamepad.y_neg, travel); break;
    case 134: hid_gamepad_keep_max_travel(&state->gamepad.y_pos, travel); break;
    case 135: hid_gamepad_keep_max_travel(&state->gamepad.x_neg, travel); break;
    case 136: hid_gamepad_keep_max_travel(&state->gamepad.x_pos, travel); break;
    case 137: hid_gamepad_keep_max_travel(&state->gamepad.extra1_neg, travel); break;
    case 138: hid_gamepad_keep_max_travel(&state->gamepad.extra1_pos, travel); break;
    case 139: hid_gamepad_keep_max_travel(&state->gamepad.extra0_neg, travel); break;
    case 140: hid_gamepad_keep_max_travel(&state->gamepad.extra0_pos, travel); break;
    case HID_GAMEPAD_CODE_TRIGGER_L:
        hid_gamepad_keep_max_travel(&state->gamepad.unipolar0, travel);
        break;
    case HID_GAMEPAD_CODE_TRIGGER_R:
        hid_gamepad_keep_max_travel(&state->gamepad.unipolar1, travel);
        break;
    case 145: hid_gamepad_keep_max_travel(&state->gamepad.unipolar2, travel); break;
    case 146: hid_gamepad_keep_max_travel(&state->gamepad.unipolar3, travel); break;
    case 147: hid_gamepad_keep_max_travel(&state->gamepad.unipolar4, travel); break;
    case 148: hid_gamepad_keep_max_travel(&state->gamepad.rx_neg, travel); break;
    case 149: hid_gamepad_keep_max_travel(&state->gamepad.rx_pos, travel); break;
    case 150: hid_gamepad_keep_max_travel(&state->gamepad.ry_neg, travel); break;
    case 151: hid_gamepad_keep_max_travel(&state->gamepad.ry_pos, travel); break;
    case 152: case 153: case 154: case 155:  /* hat up, down, left, right */
        hid_gamepad_or_button(&state->dpad,
                              (uint8_t)(1u << (code - HID_GAMEPAD_CODE_HAT_FIRST)), pressed);
        break;
    default:
        return false;
    }
    return true;
}

/* Merges one key bound to gamepad control `code` (120..155) into the per-pass accumulator:
 * buttons are ORed with `pressed`, axis directions keep the maximum travel (0..4095).
 * has_input is set when the key is pressed or has travel and the code is valid for the USB
 * mode; in the modes without a gamepad interface every code counts. */
void hid_gamepad_apply_binding(hid_gamepad_input_t *state, uint32_t code, uint32_t travel, uint32_t pressed)
{
    bool active = (pressed != 0) || (travel != 0);
    uint8_t usb_mode = g_usb_link_flags & USB_LINK_MODE_MASK;

    if (usb_mode == USB_MODE_XINPUT) {
        if (!hid_gamepad_apply_xinput_binding(state, code, travel, pressed)) {
            return;
        }
    } else if (usb_mode == USB_MODE_GAMEPAD) {
        if (!hid_gamepad_apply_classic_binding(state, code, travel, pressed)) {
            return;
        }
    }

    if (active) {
        state->has_input = 1;
    }
}

/* Key travel 0..4095 -> axis input 0..1. With loaded gamepad settings the value is rescaled
 * to the x range of the response curve: below the first point 0, above the last point 1. */
float hid_gamepad_normalize_travel(uint32_t travel)
{
    float t = fminf((float)travel / HID_GAMEPAD_TRAVEL_MAX, 1.0f);

    if (!hid_gamepad_settings_loaded()) {
        return t;
    }

    const GamepadCurvePoint *points = g_hid_gamepad_settings.response_curve.points;
    float range_start = (float)points[CURVE_POINT_DEADZONE].x / HID_GAMEPAD_CURVE_SCALE;
    if (t < range_start) {
        return 0.0f;
    }
    float range_end = (float)points[CURVE_POINT_SATURATION].x / HID_GAMEPAD_CURVE_SCALE;
    if (t > range_end) {
        return 1.0f;
    }
    return (t - range_start) / (range_end - range_start);
}

/* Combines the two opposing directions of one axis into -1..1. Default: pos - neg. With the
 * option "single direction axes" only one direction counts: the deeper one; a direction that
 * is bottomed out beats one that is not; when both are bottomed out (or exactly equal) the
 * direction that was not winning before takes over. *priority_state (enum
 * hid_gamepad_axis_socd) remembers the winner between passes. */
float hid_gamepad_combine_axis(uint32_t travel_pos, uint32_t travel_neg, uint8_t *priority_state)
{
    float pos = (travel_pos != 0) ? hid_gamepad_normalize_travel(travel_pos) : 0.0f;
    float neg = (travel_neg != 0) ? hid_gamepad_normalize_travel(travel_neg) : 0.0f;

    if (!hid_gamepad_settings_loaded()
        || !g_hid_gamepad_settings.stick_options.has_single_direction_axes
        || !g_hid_gamepad_settings.stick_options.single_direction_axes) {
        return pos - neg;
    }

    bool contested;     /* neither direction wins on travel alone */
    bool pos_wins = false;

    if (travel_pos == 0) {
        if (travel_neg == 0) {
            *priority_state = HID_AXIS_SOCD_NONE;
            return pos;  /* 0.0f */
        }
        contested = false;
    } else if (travel_pos <= HID_AXIS_SOCD_FULL_TRAVEL) {
        if (travel_neg <= HID_AXIS_SOCD_FULL_TRAVEL) {
            contested = (travel_pos == travel_neg);
            pos_wins = (travel_pos > travel_neg);
        } else {
            contested = false;
        }
    } else {
        contested = (travel_neg > HID_AXIS_SOCD_FULL_TRAVEL);
        pos_wins = true;
    }

    if (!contested) {
        if (pos_wins) {
            *priority_state = HID_AXIS_SOCD_POS;
            return pos;
        }
        *priority_state = HID_AXIS_SOCD_NEG;
        return -neg;
    }

    switch (*priority_state) {
    case HID_AXIS_SOCD_POS:
        *priority_state = HID_AXIS_SOCD_NEG_TOOK_OVER;
        return -neg;
    case HID_AXIS_SOCD_NEG:
        *priority_state = HID_AXIS_SOCD_POS_TOOK_OVER;
        return pos;
    case HID_AXIS_SOCD_POS_TOOK_OVER:
        return pos;
    case HID_AXIS_SOCD_NEG_TOOK_OVER:
        return -neg;
    default:
        /* Both directions arrived in the same pass: no winner yet. */
        return 0.0f;
    }
}

/* Maps a normalised axis input t (0..1) through the 4-point piecewise-linear response curve
 * of the gamepad settings; returns t unchanged when no settings are loaded. The x of the
 * two inner points is taken relative to the input range [x0, x3]. */
float hid_gamepad_apply_response_curve(float t)
{
    if (!hid_gamepad_settings_loaded()) {
        return t;
    }

    const GamepadCurvePoint *points = g_hid_gamepad_settings.response_curve.points;
    uint8_t x0 = points[CURVE_POINT_DEADZONE].x;
    float range = (float)(uint8_t)(points[CURVE_POINT_SATURATION].x - x0) / HID_GAMEPAD_CURVE_SCALE;
    float knee_low = ((float)(uint8_t)(points[CURVE_POINT_KNEE_LOW].x - x0) / HID_GAMEPAD_CURVE_SCALE) / range;
    float y1 = (float)points[CURVE_POINT_KNEE_LOW].y / HID_GAMEPAD_CURVE_SCALE;
    float out;

    if (knee_low > t) {
        float y0 = (float)points[CURVE_POINT_DEADZONE].y / HID_GAMEPAD_CURVE_SCALE;
        out = y0 + t * ((y1 - y0) / knee_low);
    } else {
        float knee_high = ((float)(uint8_t)(points[CURVE_POINT_KNEE_HIGH].x - x0) / HID_GAMEPAD_CURVE_SCALE) / range;
        float y2 = (float)points[CURVE_POINT_KNEE_HIGH].y / HID_GAMEPAD_CURVE_SCALE;

        if (t < knee_high) {
            out = y1 + ((y2 - y1) / (knee_high - knee_low)) * (t - knee_low);
        } else {
            float y3 = (float)points[CURVE_POINT_SATURATION].y / HID_GAMEPAD_CURVE_SCALE;

            if (t < 1.0f) {
                out = y2 + ((y3 - y2) / (1.0f - knee_high)) * (t - knee_high);
            } else {
                out = y3;
            }
        }
    }
    return fminf(out, 1.0f);
}

/* Shapes the two axis values (-1..1) of one stick in place. The magnitude is the response
 * curve applied to max(|x|, |y|); below min_magnitude (one output step) both become 0. The
 * direction is the input vector divided by its length (circular) or by its larger component
 * (square, by the settings). Before that both components are halved, or, for the left stick
 * pointing up (y < 0) with the "up bias" option, weighted p / (1 - p), which turns the
 * diagonals. */
void hid_gamepad_shape_stick(float *x, float *y, uint32_t is_left_stick, float min_magnitude)
{
    float vx = *x;
    float vy = *y;
    float largest = fmaxf(fabsf(vx), fabsf(vy));

    if (largest < min_magnitude) {
        *y = 0.0f;
        *x = 0.0f;
        return;
    }

    float magnitude = (largest != 0.0f) ? hid_gamepad_apply_response_curve(largest) : 0.0f;

    const GamepadStickOptions *options = &g_hid_gamepad_settings.stick_options;
    float weight_x = 0.5f;
    float weight_y = 0.5f;

    if (is_left_stick != 0 && vy < 0.0f && hid_gamepad_settings_loaded()
        && options->has_left_stick_up_x_percent && options->left_stick_up_bias_enabled) {
        weight_x = (float)(int32_t)options->left_stick_up_x_percent / HID_GAMEPAD_PERCENT_SCALE;
        weight_y = 1.0f - weight_x;
    }
    vx = vx * weight_x;
    vy = vy * weight_y;

    float length;
    if (hid_gamepad_settings_loaded() && options->has_normalization
        && options->normalization == StickNormalization_STICK_NORMALIZATION_SQUARE) {
        length = fmaxf(fabsf(vx), fabsf(vy));
    } else {
        /* Operand order matters: GCC contracts this into a fused multiply-add of vx * vx
         * onto the product vy * vy, so vy * vy is the one that is rounded on its own. */
        length = sqrtf(vx * vx + vy * vy);
    }

    float unit_x;
    float unit_y;
    if (length != 0.0f) {
        unit_x = vx / length;
        unit_y = vy / length;
    } else {
        unit_x = 0.0f;
        unit_y = 0.0f;
    }

    *x = unit_x * magnitude;
    *y = unit_y * magnitude;
}
