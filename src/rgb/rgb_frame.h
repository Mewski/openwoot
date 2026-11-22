/*
 * Public functions of the LED frame unit (rgb_frame*.c, rgb_brightness.c): LED number ->
 * frame position, per-LED writes, brightness and the output stage, the RGB task step. The
 * overlay functions are declared in rgb_overlay.h, which this header includes for its callers.
 *
 * Depends on rgb/rgb_types.h, rgb/rgb_data.h and the gamma table (tables/rgb_color_tables.h).
 */
#ifndef RGB_RGB_FRAME_H_
#define RGB_RGB_FRAME_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "rgb/rgb_data.h"
#include "rgb/rgb_overlay.h"
#include "rgb/rgb_types.h"
#include "tables/rgb_color_tables.h"

/* Output value of a linear channel value: gamma curve, then scaling by the (gamma corrected)
 * brightness. */
static inline uint8_t rgb_output_value(uint8_t linear, uint16_t brightness_scale)
{
    return (uint8_t)((uint16_t)(brightness_scale + k_rgb_gamma_table[linear] * brightness_scale) >> 8);
}

void rgb_update_output_enable(void);
void rgb_save_settings_timer_cb(void);
void rgb_apply_brightness_change(void);
void rgb_paint_base_colors(uint32_t force);
void rgb_wake_led_driver(void);
void rgb_update_frame(void);
void rgb_reset_idle_timer(void);
void rgb_step_brightness(uint32_t increase);
void rgb_brightness_up_timer_cb(void);
void rgb_brightness_down_timer_cb(void);
int rgb_frame_locate_led(rgb_frame_t *frame, uint32_t led_id, uint8_t *page_r, uint8_t *page_g, uint8_t *page_b, uint8_t **ch_r, uint8_t **ch_g, uint8_t **ch_b);
uint32_t rgb_blend_argb(uint32_t from, uint32_t to, uint32_t t);
void rgb_mark_page_dirty(uint32_t page);
void rgb_frame_set_led(rgb_frame_t *frame, uint32_t led_id, uint8_t r, uint8_t g, uint8_t b);
void rgb_frame_set_key_scaled(rgb_frame_t *frame, uint32_t row, uint32_t col, uint8_t r, uint8_t g, uint8_t b, uint8_t scale);
void rgb_frame_fill_keys(rgb_frame_t *frame, uint8_t r, uint8_t g, uint8_t b);
void rgb_frame_scale_led(rgb_frame_t *frame, uint32_t led_id, uint32_t scale);
uint32_t rgb_unpack_color(uint32_t packed);
uint32_t rgb_convert_rgb565_to_argb(uint16_t rgb565);
/* Colour of LED led_id in frame as RGB565. `last` carries the colour of the previously read LED:
 * an LED number the frame cannot locate (RGB_LED_NONE) repeats it. */
uint16_t rgb_frame_read_led_rgb565(rgb_frame_t *frame, uint32_t led_id, RgbColor *last);

#endif /* RGB_RGB_FRAME_H_ */
