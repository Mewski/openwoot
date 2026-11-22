/*
 * rgb_frame_layout.h - how the LEDs of a board sit in the LED frame: the LED numbering and the
 * element types of the two board tables that describe the frame, k_rgb_led_channels (LED
 * number -> frame page and channel offset) and k_rgb_frame_pages (frame page -> run of driver
 * registers). A board's tables/rgb_tables.c defines both; this header is what they and the
 * frame code agree on.
 *
 * Depends on board.h only, so that the board tables can include it.
 */
#ifndef RGB_FRAME_LAYOUT_H
#define RGB_FRAME_LAYOUT_H

#include <stdint.h>

#include "board.h"

/* LED numbers run from 0 to RGB_LED_LIMIT - 1; RGB_LED_NONE is the "no LED" entry of the key ->
 * LED tables. Numbers without an LED have the entry RGB_LED_CHANNEL_NONE in k_rgb_led_channels. */
#define RGB_LED_LIMIT              BOARD_LED_LIMIT
#define RGB_LED_NONE               BOARD_LED_NONE

/* Frame pages; each holds the PWM values of one contiguous run of driver registers. */
#define RGB_FRAME_PAGE_COUNT       BOARD_RGB_FRAME_PAGE_COUNT
#define RGB_FRAME_PAGE_PWM_MAX     BOARD_RGB_FRAME_PAGE_PWM_MAX /* PWM bytes of the longest page */

/*
 * Where the three channels of one LED are in the frame: frame page and offset of the red byte
 * in that page's pwm[]; green and blue are the two bytes after it.
 */
typedef struct {
    uint8_t page;   /* frame page 0..RGB_FRAME_PAGE_COUNT - 1, RGB_LED_PAGE_NONE = no LED */
    uint8_t offset; /* pwm[] offset of the red channel */
} rgb_led_channel_t;

#define RGB_LED_PAGE_NONE          0xFF
#define RGB_LED_CHANNEL_NONE       { RGB_LED_PAGE_NONE, 0 }

/* The run of driver registers that one frame page covers. */
typedef struct {
    uint8_t bank;      /* register bank of the LED driver (led_matrix_write) */
    uint8_t first_reg; /* first PWM register of the run */
    uint8_t length;    /* PWM registers in the run, at most RGB_FRAME_PAGE_PWM_MAX */
} rgb_frame_page_layout_t;

#endif /* RGB_FRAME_LAYOUT_H */
