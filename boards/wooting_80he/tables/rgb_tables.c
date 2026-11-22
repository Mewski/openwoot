/*
 * rgb_tables.c - lighting data of the Wooting 80HE (definitions): key -> LED maps per layout
 * variant and the layout of the LED frame (where each LED number sits in the frame, which driver
 * registers each frame page covers). The light-bar LEDs are BOARD_LIGHTBAR_LEDS of board.h.
 *
 * LED numbers are 1..104 (eight switch lines of 13 LEDs); 0x69 marks a key without LED.
 */
#include "rgb_tables.h"

/* Key [row][col] -> LED number 1..104, 0x69 = no LED; ISO variant */
const uint8_t k_rgb_key_to_led_iso[BOARD_KEY_ROWS][BOARD_KEY_COLS] = {
    {   1,  14,  27,  40,  53,  66,  79,  92,   7, 105,  20,  33,  46,  59,  72,  85,  98 },
    {   2,  15,  28,  41,  54,  67,  80,  93,   8,  21,  34,  47,  60, 105,  73,  86,  99 },
    {   3,  16,  29,  42,  55,  68,  81,  94,   9,  22,  35,  48,  61, 105, 105,  87, 100 },
    {   4,  17,  30,  43,  56,  69,  82,  95,  10,  23,  36,  49,  62, 105,  75, 105, 105 },
    {   5,  18,  31,  44,  57,  70,  83,  96,  11,  24,  37,  50, 105, 105,  63,  76, 105 },
    {   6,  19,  32, 105,  45,  58,  71,  84,  97, 105,  12,  25,  38, 105,  13,  26,  39 },
};

/* Key [row][col] -> LED number 1..104, 0x69 = no LED; JIS variant */
const uint8_t k_rgb_key_to_led_jis[BOARD_KEY_ROWS][BOARD_KEY_COLS] = {
    {   1,  14,  27,  40,  53,  66,  79,  92,   7, 105,  20,  33,  46,  59,  72,  85,  98 },
    {   2,  15,  28,  41,  54,  67,  80,  93,   8,  21,  34,  47,  60,  73,  74,  86,  99 },
    {   3,  16,  29,  42,  55,  68,  81,  94,   9,  22,  35,  48,  61, 105, 105,  87, 100 },
    {   4,  17,  30,  43,  56,  69,  82,  95,  10,  23,  36,  49,  62, 105,  75, 105, 105 },
    {   5, 105,  31,  44,  57,  70,  83,  96,  11,  24,  37,  50,  63, 105,  89,  76, 105 },
    {   6,  18,  19,  32, 105,  58,  71,  84, 105,  97,  12,  25,  38, 105,  13,  26,  39 },
};

/* Key [row][col] -> LED number 1..104, 0x69 = no LED; ANSI variant (also used for unknown variants) */
const uint8_t k_rgb_key_to_led_ansi[BOARD_KEY_ROWS][BOARD_KEY_COLS] = {
    {   1,  14,  27,  40,  53,  66,  79,  92,   7, 105,  20,  33,  46,  59,  72,  85,  98 },
    {   2,  15,  28,  41,  54,  67,  80,  93,   8,  21,  34,  47,  60, 105,  73,  86,  99 },
    {   3,  16,  29,  42,  55,  68,  81,  94,   9,  22,  35,  48,  61, 105,  62,  87, 100 },
    {   4,  17,  30,  43,  56,  69,  82,  95,  10,  23,  36,  49, 105, 105,  75, 105, 105 },
    {   5, 105,  31,  44,  57,  70,  83,  96,  11,  24,  37,  50, 105, 105,  63,  76, 105 },
    {   6,  19,  32, 105,  45,  58,  71,  84,  97, 105,  12,  25,  38, 105,  13,  26,  39 },
};

/*
 * LED number -> frame page and offset of its red channel. LED n is on switch line (n - 1) / 13;
 * channels 1..30 of a line are in the frame page of that line, channels 31..39 in page 8 or 9.
 * Number 0 carries no LED.
 */
const rgb_led_channel_t k_rgb_led_channels[BOARD_LED_LIMIT] = {
    [0] = RGB_LED_CHANNEL_NONE,
    /* 1..13: switch line 1 */
    { 0, 15 }, { 0, 12 }, { 0,  9 }, { 0,  6 }, { 0,  3 }, { 0,  0 }, { 0, 18 },
    { 8,  6 }, { 8,  3 }, { 0, 21 }, { 8,  0 }, { 0, 27 }, { 0, 24 },
    /* 14..26: switch line 2 */
    { 1, 15 }, { 1, 12 }, { 1,  9 }, { 1,  6 }, { 1,  3 }, { 1,  0 }, { 1, 18 },
    { 8, 15 }, { 8, 12 }, { 1, 21 }, { 8,  9 }, { 1, 27 }, { 1, 24 },
    /* 27..39: switch line 3 */
    { 2, 15 }, { 2, 12 }, { 2,  9 }, { 2,  6 }, { 2,  3 }, { 2,  0 }, { 2, 18 },
    { 8, 24 }, { 8, 21 }, { 2, 21 }, { 8, 18 }, { 2, 27 }, { 2, 24 },
    /* 40..52: switch line 4 */
    { 3, 15 }, { 3, 12 }, { 3,  9 }, { 3,  6 }, { 3,  3 }, { 3,  0 }, { 3, 18 },
    { 8, 33 }, { 8, 30 }, { 3, 21 }, { 8, 27 }, { 3, 27 }, { 3, 24 },
    /* 53..65: switch line 5 */
    { 4, 15 }, { 4, 12 }, { 4,  9 }, { 4,  6 }, { 4,  3 }, { 4,  0 }, { 4, 18 },
    { 9,  6 }, { 9,  3 }, { 4, 21 }, { 9,  0 }, { 4, 27 }, { 4, 24 },
    /* 66..78: switch line 6 */
    { 5, 15 }, { 5, 12 }, { 5,  9 }, { 5,  6 }, { 5,  3 }, { 5,  0 }, { 5, 18 },
    { 9, 15 }, { 9, 12 }, { 5, 21 }, { 9,  9 }, { 5, 27 }, { 5, 24 },
    /* 79..91: switch line 7 */
    { 6, 15 }, { 6, 12 }, { 6,  9 }, { 6,  6 }, { 6,  3 }, { 6,  0 }, { 6, 18 },
    { 9, 24 }, { 9, 21 }, { 6, 21 }, { 9, 18 }, { 6, 27 }, { 6, 24 },
    /* 92..104: switch line 8 */
    { 7, 15 }, { 7, 12 }, { 7,  9 }, { 7,  6 }, { 7,  3 }, { 7,  0 }, { 7, 18 },
    { 9, 33 }, { 9, 30 }, { 7, 21 }, { 9, 27 }, { 7, 27 }, { 7, 24 },
};

/* Frame page -> driver register bank, first PWM register, number of registers. */
const rgb_frame_page_layout_t k_rgb_frame_pages[BOARD_RGB_FRAME_PAGE_COUNT] = {
    { 0,   0, 30 }, /* 0: switch line 1, channels 1..30 */
    { 0,  30, 30 }, /* 1: switch line 2 */
    { 0,  60, 30 }, /* 2: switch line 3 */
    { 0,  90, 30 }, /* 3: switch line 4 */
    { 0, 120, 30 }, /* 4: switch line 5 */
    { 0, 150, 30 }, /* 5: switch line 6 */
    { 1,   0, 30 }, /* 6: switch line 7 */
    { 1,  30, 30 }, /* 7: switch line 8 */
    { 1,  90, 36 }, /* 8: channels 31..39 of switch lines 1..4, 9 per line */
    { 1, 126, 36 }, /* 9: channels 31..39 of switch lines 5..8 */
};
