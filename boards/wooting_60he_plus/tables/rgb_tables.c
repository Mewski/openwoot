/*
 * rgb_tables.c - lighting data of the Wooting 60HE+ (definitions): key -> LED maps per layout
 * variant and the layout of the LED frame (where each LED number sits in the frame, which
 * driver registers each frame page covers).
 *
 * LED numbers are 0..107: ten LEDs on channels 1..30 of switch lines 1..5 (0..49) and three on
 * channels 31..39 of switch lines 1..6 (90..107); 0x6C marks a key without LED. Row 0 of the key
 * matrix carries no key.
 */
#include "rgb_tables.h"

/* Key [row][col] -> LED number 0..107, 0x6C = no LED; ISO variant. Differs from the ANSI table at
 * (2, 13), (3, 12) and (4, 1). */
const uint8_t k_rgb_key_to_led_iso[BOARD_KEY_ROWS][BOARD_KEY_COLS] = {
    { 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108 },
    {   0,   1,   2,   3,   4,   5,   6,   7,   8,   9,  90,  91,  92, 107 },
    {  10,  11,  12,  13,  14,  15,  16,  17,  18,  19,  93,  94,  95, 108 },
    {  20,  21,  22,  23,  24,  25,  26,  27,  28,  29,  96,  97,  98, 104 },
    {  30,  31,  32,  33,  34,  35,  36,  37,  38,  39,  99, 100, 108, 101 },
    {  40,  41,  42, 108,  43,  44,  45,  46,  47, 108,  48,  49, 102, 103 },
};

/* Key [row][col] -> LED number 0..107, 0x6C = no LED; ANSI variant (also used for unknown variants) */
const uint8_t k_rgb_key_to_led_ansi[BOARD_KEY_ROWS][BOARD_KEY_COLS] = {
    { 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108, 108 },
    {   0,   1,   2,   3,   4,   5,   6,   7,   8,   9,  90,  91,  92, 107 },
    {  10,  11,  12,  13,  14,  15,  16,  17,  18,  19,  93,  94,  95,  98 },
    {  20,  21,  22,  23,  24,  25,  26,  27,  28,  29,  96,  97, 108, 104 },
    {  30, 108,  32,  33,  34,  35,  36,  37,  38,  39,  99, 100, 108, 101 },
    {  40,  41,  42, 108,  43,  44,  45,  46,  47, 108,  48,  49, 102, 103 },
};

/* LED number -> frame page and offset of its red channel; numbers 50..89 carry no LED. */
const rgb_led_channel_t k_rgb_led_channels[BOARD_LED_LIMIT] = {
    /* 0..9: switch line 1, channels 1..30 */
    { 0,  0 }, { 0,  3 }, { 0,  6 }, { 0,  9 }, { 0, 12 }, { 0, 15 }, { 0, 18 }, { 0, 21 }, { 0, 24 }, { 0, 27 },
    /* 10..19: switch line 2, channels 1..30 */
    { 1,  0 }, { 1,  3 }, { 1,  6 }, { 1,  9 }, { 1, 12 }, { 1, 15 }, { 1, 18 }, { 1, 21 }, { 1, 24 }, { 1, 27 },
    /* 20..29: switch line 3, channels 1..30 */
    { 2,  0 }, { 2,  3 }, { 2,  6 }, { 2,  9 }, { 2, 12 }, { 2, 15 }, { 2, 18 }, { 2, 21 }, { 2, 24 }, { 2, 27 },
    /* 30..39: switch line 4, channels 1..30 */
    { 3,  0 }, { 3,  3 }, { 3,  6 }, { 3,  9 }, { 3, 12 }, { 3, 15 }, { 3, 18 }, { 3, 21 }, { 3, 24 }, { 3, 27 },
    /* 40..49: switch line 5, channels 1..30 */
    { 4,  0 }, { 4,  3 }, { 4,  6 }, { 4,  9 }, { 4, 12 }, { 4, 15 }, { 4, 18 }, { 4, 21 }, { 4, 24 }, { 4, 27 },
    [50 ... 89] = RGB_LED_CHANNEL_NONE,
    /* 90..98: channels 31..39 of switch lines 1..3 */
    [90] = { 5,  0 }, { 5,  3 }, { 5,  6 }, { 5,  9 }, { 5, 12 }, { 5, 15 }, { 5, 18 }, { 5, 21 }, { 5, 24 },
    /* 99..107: channels 31..39 of switch lines 4..6 */
    { 6,  0 }, { 6,  3 }, { 6,  6 }, { 6,  9 }, { 6, 12 }, { 6, 15 }, { 6, 18 }, { 6, 21 }, { 6, 24 },
};

/* Frame page -> driver register bank, first PWM register, number of registers. */
const rgb_frame_page_layout_t k_rgb_frame_pages[BOARD_RGB_FRAME_PAGE_COUNT] = {
    { 0,   0, 30 }, /* 0: switch line 1, channels 1..30 */
    { 0,  30, 30 }, /* 1: switch line 2 */
    { 0,  60, 30 }, /* 2: switch line 3 */
    { 0,  90, 30 }, /* 3: switch line 4 */
    { 0, 120, 30 }, /* 4: switch line 5 */
    { 1,  90, 27 }, /* 5: channels 31..39 of switch lines 1..3, 9 per line */
    { 1, 117, 27 }, /* 6: channels 31..39 of switch lines 4..6 */
};
