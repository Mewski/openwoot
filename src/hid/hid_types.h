/*
 * hid_types.h - types and constants of the `hid` module (HID / XInput report building).
 */
#ifndef HID_TYPES_H
#define HID_TYPES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* ---- Sizes ------------------------------------------------------------------------------- */

#define HID_BOOT_KBD_REPORT_SIZE     8u   /* interface 1, 6KRO / boot protocol */
#define HID_BOOT_KBD_KEYS            6u
#define HID_NKRO_REPORT_SIZE         28u  /* interface 1, NKRO */
#define HID_NKRO_BITMAP_SIZE         27u
#define HID_EXTRA_REPORT_SIZE        7u   /* interface 3 */
#define HID_EXTRA_CONSUMER_USAGES    3u   /* g_hid_consumer_usage_count limit */
#define HID_KEY_STATE_ENTRIES        16u  /* further entries of a pass are dropped */
#define HID_KEY_STATE_REPORT_SIZE    64u  /* interface 4 */
#define HID_GAMEPAD_REPORT_SIZE      25u  /* interface 0 in USB mode 1 */
#define HID_XINPUT_PAD_STATE_SIZE    12u  /* argument of hid_send_xinput_report */
#define HID_XINPUT_REPORT_SIZE       20u  /* 0x14, transfer length of the XInput IN report */
#define HID_GAMEPAD_STICK_AXES       4u   /* g_hid_gamepad_stick_axis_socd */
#define HID_GAMEPAD_EXTRA_AXES       2u   /* g_hid_gamepad_extra_axis_socd */

/* ---- Keyboard reports (HID interface 1, no report id) -------------------------------------- */

/* 6KRO / boot keyboard report, 8 bytes. Filled by keys_apply_key_action, sent while
 * g_run_state_flags bit 2 is set. */
typedef struct hid_boot_kbd_report {
    uint8_t modifiers;                   /* +0x00 bit n = usage 0xE0 + n */
    uint8_t reserved;                    /* +0x01 never written (stays 0) */
    uint8_t keys[HID_BOOT_KBD_KEYS];     /* +0x02 usages, 0 = free slot */
} hid_boot_kbd_report_t;

_Static_assert(sizeof(hid_boot_kbd_report_t) == 8, "hid_boot_kbd_report_t size");
_Static_assert(offsetof(hid_boot_kbd_report_t, keys) == 2, "hid_boot_kbd_report_t.keys");

/* NKRO keyboard report, 28 bytes.
 * Usage u in 4..0x32: bitmap[(u - 4) / 8] bit (u - 4) % 8 (report bytes 1..6);
 * u >= 0x33: bitmap[6 + (u - 0x33) / 8] bit (u - 0x33) % 8 (report bytes 7..27).
 * Bit 7 of report byte 6 is never set, usages 0..3 are dropped. */
typedef struct hid_nkro_report {
    uint8_t modifiers;                     /* +0x00 */
    uint8_t bitmap[HID_NKRO_BITMAP_SIZE];  /* +0x01 */
} hid_nkro_report_t;

_Static_assert(sizeof(hid_nkro_report_t) == 28, "hid_nkro_report_t size");
_Static_assert(offsetof(hid_nkro_report_t, bitmap) == 1, "hid_nkro_report_t.bitmap");

/* ---- System control / consumer / mouse-button report (HID interface 3) --------------------- */

/* Value of hid_extra_report_t.report_id. */
enum hid_extra_report_id {
    HID_EXTRA_REPORT_SYSTEM   = 1,  /* data[0] = system control code 1..3 */
    HID_EXTRA_REPORT_CONSUMER = 2,  /* up to three 16-bit consumer usages */
    HID_EXTRA_REPORT_BUTTONS  = 3,  /* data[0] = bitmap, bit = code - 4 (codes 4..11) */
};

/* 7 bytes: report id + 6 data bytes. The id byte is never reset between passes, so an
 * "all released" report carries the id of the last use. */
typedef struct __attribute__((packed)) hid_extra_report {
    uint8_t report_id;                               /* +0x00 enum hid_extra_report_id */
    union __attribute__((packed)) {
        uint8_t  data[6];                            /* +0x01 */
        uint8_t  system_code;                        /* +0x01 id 1 */
        uint8_t  buttons;                            /* +0x01 id 3 */
        uint16_t consumer[HID_EXTRA_CONSUMER_USAGES];/* +0x01 id 2: low byte code, high byte usage page part */
    };
} hid_extra_report_t;

_Static_assert(sizeof(hid_extra_report_t) == 7, "hid_extra_report_t size");
_Static_assert(offsetof(hid_extra_report_t, data) == 1, "hid_extra_report_t.data");
_Static_assert(offsetof(hid_extra_report_t, consumer) == 1, "hid_extra_report_t.consumer");

/* ---- Key-state report (HID interface 4, 64-byte vendor-defined input) ---------------------- */

/* Value of hid_key_state_entry_t.kind. */
enum hid_key_state_kind {
    HID_KEY_STATE_KIND_KEYBOARD = 0,  /* action types 0 and 1 (usage / modifier) */
    HID_KEY_STATE_KIND_CONSUMER = 3,
    HID_KEY_STATE_KIND_FN       = 4,  /* Fn action or "no binding" */
    HID_KEY_STATE_KIND_GAMEPAD  = 5,
    HID_KEY_STATE_KIND_ADVANCED = 6,  /* advanced key event */
};

/* One entry of the key-state report. Written by hid_keystate_report_add_key and by
 * keys_apply_key_action / keys_process_scan_pass. */
typedef struct hid_key_state_entry {
    uint8_t  key_id;        /* +0x00 matrix position byte (row << 5 | col) */
    uint8_t  code;          /* +0x01 usage; 0xE0 + n for modifiers (0xE8 if the mask is 0); raw code otherwise */
    uint16_t pressed : 1;   /* +0x02 bit 0 */
    uint16_t unk_bit1 : 1;  /*       bit 1: never written by hid_keystate_report_add_key */
    uint16_t kind : 4;      /*       bits 5:2 enum hid_key_state_kind */
    uint16_t travel : 10;   /*       bits 15:6 travel * 1023 / 4095 */
} hid_key_state_entry_t;

_Static_assert(sizeof(hid_key_state_entry_t) == 4, "hid_key_state_entry_t size");
_Static_assert(offsetof(hid_key_state_entry_t, code) == 1, "hid_key_state_entry_t.code");

typedef struct hid_key_state_report {
    hid_key_state_entry_t entries[HID_KEY_STATE_ENTRIES];  /* unused entries are 0 */
} hid_key_state_report_t;

_Static_assert(sizeof(hid_key_state_report_t) == HID_KEY_STATE_REPORT_SIZE, "hid_key_state_report_t size");

/* ---- Gamepad reports ----------------------------------------------------------------------- */

/* Classic HID gamepad report, 25 bytes, HID interface 0 in USB mode 1, no report id
 * (descriptor k_usb_hid_report_desc_gamepad). Built in keys_process_scan_pass, sent by
 * hid_send_gamepad_report. */
#define HID_GAMEPAD_AXIS_CENTER   0x3FFFu   /* idle value of the six bipolar axes */
#define HID_GAMEPAD_AXIS_RANGE    16383     /* +/- range around the centre */
#define HID_GAMEPAD_TRIGGER_MAX   0x7FFFu   /* full scale of the unipolar axes */
#define HID_GAMEPAD_DETECT_AXIS   0x7FFEu   /* X = Y while g_hid_gamepad_detection_active */

typedef struct __attribute__((packed)) hid_gamepad_report {
    uint16_t buttons;      /* +0x00 12 button bits */
    uint16_t x;            /* +0x02 first stick X, 0..0x7FFE, centre 0x3FFF */
    uint16_t y;            /* +0x04 first stick Y */
    uint16_t rx;           /* +0x06 second stick X */
    uint16_t ry;           /* +0x08 second stick Y */
    uint16_t extra[2];     /* +0x0A two more bipolar axes (g_hid_gamepad_extra_axis_socd) */
    uint16_t unipolar[5];  /* +0x0E five unipolar axes 0..0x7FFF */
    uint8_t  hat;          /* +0x18 1..8 clockwise from up, 0 = centred */
} hid_classic_gamepad_report_t;  /* not hid_gamepad_report_t: TinyUSB's class/hid/hid.h owns that name */

_Static_assert(sizeof(hid_classic_gamepad_report_t) == HID_GAMEPAD_REPORT_SIZE, "hid_classic_gamepad_report_t size");
_Static_assert(offsetof(hid_classic_gamepad_report_t, x) == 0x02, "hid_classic_gamepad_report_t.x");
_Static_assert(offsetof(hid_classic_gamepad_report_t, extra) == 0x0A, "hid_classic_gamepad_report_t.extra");
_Static_assert(offsetof(hid_classic_gamepad_report_t, unipolar) == 0x0E, "hid_classic_gamepad_report_t.unipolar");
_Static_assert(offsetof(hid_classic_gamepad_report_t, hat) == 0x18, "hid_classic_gamepad_report_t.hat");

/* XInput pad state, 12 bytes: what keys_process_scan_pass builds and passes to
 * hid_send_xinput_report. Naturally aligned. */
typedef struct hid_xinput_pad_state {
    uint16_t buttons;        /* +0x00 XInput wButtons */
    uint8_t  left_trigger;   /* +0x02 0..255 */
    uint8_t  right_trigger;  /* +0x03 */
    int16_t  lx;             /* +0x04 */
    int16_t  ly;             /* +0x06 positive = up */
    int16_t  rx;             /* +0x08 */
    int16_t  ry;             /* +0x0A */
} hid_xinput_pad_state_t;

_Static_assert(sizeof(hid_xinput_pad_state_t) == HID_XINPUT_PAD_STATE_SIZE, "hid_xinput_pad_state_t size");
_Static_assert(offsetof(hid_xinput_pad_state_t, lx) == 4, "hid_xinput_pad_state_t.lx");

#define HID_XINPUT_REPORT_TYPE   0x00u  /* byte 0 of the IN report */
#define HID_XINPUT_REPORT_LEN    0x14u  /* byte 1 of the IN report */

/* XInput (Xbox 360 wired pad) IN report, 20 bytes, vendor-defined interface 0 in USB mode 0.
 * hid_send_xinput_report compares / copies the first 14 bytes and zeroes the last 6. */
typedef struct hid_xinput_report {
    uint8_t  type;                 /* +0x00 0x00 */
    uint8_t  length;               /* +0x01 0x14 */
    hid_xinput_pad_state_t pad;    /* +0x02 */
    uint8_t  reserved[6];          /* +0x0E always 0 */
} hid_xinput_report_t;

_Static_assert(sizeof(hid_xinput_report_t) == HID_XINPUT_REPORT_SIZE, "hid_xinput_report_t size");
_Static_assert(offsetof(hid_xinput_report_t, pad) == 0x02, "hid_xinput_report_t.pad");
_Static_assert(offsetof(hid_xinput_report_t, reserved) == 0x0E, "hid_xinput_report_t.reserved");

/* XInput wButtons bits as set by hid_gamepad_apply_binding (USB mode 0). */
#define HID_XINPUT_BTN_DPAD_UP     0x0001u
#define HID_XINPUT_BTN_DPAD_DOWN   0x0002u
#define HID_XINPUT_BTN_DPAD_LEFT   0x0004u
#define HID_XINPUT_BTN_DPAD_RIGHT  0x0008u
#define HID_XINPUT_BTN_START       0x0010u
#define HID_XINPUT_BTN_BACK        0x0020u
#define HID_XINPUT_BTN_L3          0x0040u
#define HID_XINPUT_BTN_R3          0x0080u
#define HID_XINPUT_BTN_LB          0x0100u
#define HID_XINPUT_BTN_RB          0x0200u
#define HID_XINPUT_BTN_GUIDE       0x0400u
#define HID_XINPUT_BTN_A           0x1000u
#define HID_XINPUT_BTN_B           0x2000u
#define HID_XINPUT_BTN_X           0x4000u
#define HID_XINPUT_BTN_Y           0x8000u

/* ---- Gamepad binding codes and the per-pass input accumulator ------------------------------ */

/* Gamepad function codes (key binding byte, argument `code` of hid_gamepad_apply_binding).
 * The meaning of 133..151 depends on the USB mode, see hid_gamepad_input_t. */
#define HID_GAMEPAD_CODE_FIRST        120u
#define HID_GAMEPAD_CODE_LAST         155u
#define HID_GAMEPAD_CODE_BUTTON_FIRST 120u   /* 120..131: A, B, X, Y, Start, Back, d-pad x4, LB, RB / buttons 1..12 */
#define HID_GAMEPAD_CODE_GUIDE        132u   /* XInput only */
#define HID_GAMEPAD_CODE_AXIS_FIRST   133u   /* 133..140 stick directions */
#define HID_GAMEPAD_CODE_L3           141u   /* XInput only */
#define HID_GAMEPAD_CODE_R3           142u   /* XInput only */
#define HID_GAMEPAD_CODE_TRIGGER_L    143u
#define HID_GAMEPAD_CODE_TRIGGER_R    144u
#define HID_GAMEPAD_CODE_HAT_FIRST    152u   /* 152..155 hat up, down, left, right (mode 1 only) */

/* Per-pass gamepad input accumulator: first 0x28 bytes of the key-pass context that
 * keys_process_scan_pass keeps on its stack (keys_pass_ctx_t of the `keys` module embeds
 * it as first member). Filled by hid_gamepad_apply_binding: travel values 0..4095 keep the
 * maximum, buttons are ORed. The layout of the axis words depends on the USB mode
 * (g_usb_link_flags & 3); modes 2 and 3 only set has_input. */
typedef struct hid_gamepad_input {
    uint8_t has_input;               /* +0x00 1 = some gamepad binding was active this pass */
    uint8_t _pad_01;
    union {                          /* +0x02 .. +0x23 */
        uint16_t w[17];              /* the axis words as a plain array */
        struct {                     /* USB mode 0 (XInput) */
            uint16_t ls_up;          /* +0x02 code 133 */
            uint16_t ls_down;        /* +0x04 code 134 */
            uint16_t ls_left;        /* +0x06 code 135 */
            uint16_t ls_right;       /* +0x08 code 136 */
            uint16_t rs_up;          /* +0x0A code 137 */
            uint16_t rs_down;        /* +0x0C code 138 */
            uint16_t rs_left;        /* +0x0E code 139 */
            uint16_t rs_right;       /* +0x10 code 140 */
            uint16_t left_trigger;   /* +0x12 code 143 */
            uint16_t right_trigger;  /* +0x14 code 144 */
            uint8_t  buttons_lo;     /* +0x16 wButtons low: d-pad up/down/left/right (126..129), Start 124, Back 125, L3 141, R3 142 */
            uint8_t  buttons_hi;     /* +0x17 wButtons high: LB 130, RB 131, Guide 132, A/B/X/Y 120..123 in bits 4..7 */
        } xinput;
        struct {                     /* USB mode 1 (classic HID gamepad) */
            uint16_t x_neg;          /* +0x02 code 135 */
            uint16_t x_pos;          /* +0x04 code 136 */
            uint16_t y_neg;          /* +0x06 code 133 */
            uint16_t y_pos;          /* +0x08 code 134 */
            uint16_t rx_neg;         /* +0x0A code 148 */
            uint16_t rx_pos;         /* +0x0C code 149 */
            uint16_t extra0_neg;     /* +0x0E code 139 -> report.extra[0] */
            uint16_t extra0_pos;     /* +0x10 code 140 */
            uint16_t extra1_neg;     /* +0x12 code 137 -> report.extra[1] */
            uint16_t extra1_pos;     /* +0x14 code 138 */
            uint16_t ry_neg;         /* +0x16 code 150 */
            uint16_t ry_pos;         /* +0x18 code 151 */
            uint16_t unipolar1;      /* +0x1A code 144 -> report.unipolar[1] */
            uint16_t unipolar0;      /* +0x1C code 143 -> report.unipolar[0] */
            uint16_t unipolar2;      /* +0x1E code 145 */
            uint16_t unipolar3;      /* +0x20 code 146 */
            uint16_t unipolar4;      /* +0x22 code 147 */
        } gamepad;
    };
    uint8_t buttons_lo;              /* +0x24 mode 1: buttons 1..8 (codes 120..127) */
    uint8_t buttons_hi;              /* +0x25 mode 1: buttons 9..12 (codes 128..131) in bits 0..3 */
    uint8_t dpad;                    /* +0x26 mode 1: bit 0 up, 1 down, 2 left, 3 right (codes 152..155) */
    uint8_t _pad_27;
} hid_gamepad_input_t;

_Static_assert(sizeof(hid_gamepad_input_t) == 0x28, "hid_gamepad_input_t size");
_Static_assert(_Alignof(hid_gamepad_input_t) == 2, "hid_gamepad_input_t alignment");
_Static_assert(offsetof(hid_gamepad_input_t, w) == 0x02, "hid_gamepad_input_t.w");
_Static_assert(offsetof(hid_gamepad_input_t, xinput.buttons_lo) == 0x16, "hid_gamepad_input_t.xinput.buttons_lo");
_Static_assert(offsetof(hid_gamepad_input_t, xinput.buttons_hi) == 0x17, "hid_gamepad_input_t.xinput.buttons_hi");
_Static_assert(offsetof(hid_gamepad_input_t, gamepad.ry_neg) == 0x16, "hid_gamepad_input_t.gamepad.ry_neg");
_Static_assert(offsetof(hid_gamepad_input_t, gamepad.unipolar4) == 0x22, "hid_gamepad_input_t.gamepad.unipolar4");
_Static_assert(offsetof(hid_gamepad_input_t, buttons_lo) == 0x24, "hid_gamepad_input_t.buttons_lo");
_Static_assert(offsetof(hid_gamepad_input_t, buttons_hi) == 0x25, "hid_gamepad_input_t.buttons_hi");
_Static_assert(offsetof(hid_gamepad_input_t, dpad) == 0x26, "hid_gamepad_input_t.dpad");

/* Opposing-direction resolver state of one gamepad axis (third argument of
 * hid_gamepad_combine_axis; elements of g_hid_gamepad_stick_axis_socd and
 * g_hid_gamepad_extra_axis_socd). */
enum hid_gamepad_axis_socd {
    HID_AXIS_SOCD_NONE          = 0,  /* neither direction pressed */
    HID_AXIS_SOCD_POS           = 1,  /* positive direction wins */
    HID_AXIS_SOCD_NEG           = 2,  /* negative direction wins */
    HID_AXIS_SOCD_POS_TOOK_OVER = 3,  /* both bottomed out, positive pressed last */
    HID_AXIS_SOCD_NEG_TOOK_OVER = 4,  /* both bottomed out, negative pressed last */
};

#define HID_AXIS_SOCD_FULL_TRAVEL  4013u  /* 0xFAD: travel above this counts as bottomed out */

/* g_hid_gamepad_file_flags bits. */
#define HID_GAMEPAD_FILE_GM_LOADED  0x01u  /* profile file "gm" (gamepad mapping) applied */
#define HID_GAMEPAD_FILE_GP_LOADED  0x02u  /* profile file "gp" loaded into g_hid_gamepad_settings */

/* g_hid_xinput_player_index value when the host has not sent an LED command. */
#define HID_XINPUT_PLAYER_UNKNOWN   0xFFu

/* g_hid_gamepad_mapped_key_count above this selects the dense reply of
 * protocol_cmd_get_gamepad_mapping. */
#define HID_GAMEPAD_DENSE_REPLY_MIN_KEYS  50u

#endif /* HID_TYPES_H */
