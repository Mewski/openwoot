/*
 * rgb_frame.c - LED frame access: the page layout of the main frame, LED number -> frame bytes,
 * per-LED writes with dirty-page tracking, key helpers and the base colour map.
 *
 * A frame (rgb_frame_t) holds RGB_FRAME_PAGE_COUNT pages of linear 8-bit PWM values; where
 * each LED sits is the board table k_rgb_led_channels. Only writes into g_rgb_frame_main queue
 * pages for transmission; every other frame is a scratch buffer.
 */
#include "rgb/rgb_frame.h"

#include "core/run_state.h"
#include "core/startup.h"
#include "hal/hal_irq.h"
#include "rgb/lightbar/lightbar.h"
#include "tables/rgb_tables.h"

/*
 * Start-up step: records which run of driver PWM registers each page of the main frame covers
 * (k_rgb_frame_pages).
 */
static void rgb_frame_pages_init(void)
{
    for (uint32_t page = 0; page < RGB_FRAME_PAGE_COUNT; page++) {
        rgb_frame_page_t *out = &g_rgb_frame_main.pages[page];

        out->index = (uint8_t)page;
        out->driver_page = k_rgb_frame_pages[page].bank;
        out->first_reg = k_rgb_frame_pages[page].first_reg;
        out->length = k_rgb_frame_pages[page].length;
    }
}
STARTUP_STEP(STARTUP_FRAME_PAGES, rgb_frame_pages_init);

/*
 * Finds the frame bytes of an LED: page numbers and pointers to the red, green and blue PWM
 * bytes of LED led_id (k_rgb_led_channels). Returns 1 on success, 0 for a number without LED
 * (RGB_LED_NONE and every number from RGB_LED_LIMIT on included) and for a page that would lie
 * at address 0 (a NULL frame and page 0; matches stock firmware 2.14.1).
 */
int rgb_frame_locate_led(rgb_frame_t *frame, uint32_t led_id, uint8_t *page_r, uint8_t *page_g, uint8_t *page_b,
                         uint8_t **ch_r, uint8_t **ch_g, uint8_t **ch_b)
{
    const rgb_led_channel_t *channel;
    uint8_t *pwm;

    if (led_id >= RGB_LED_LIMIT) {
        return 0;
    }
    channel = &k_rgb_led_channels[led_id];
    if (channel->page == RGB_LED_PAGE_NONE) {
        return 0;
    }
    if ((uintptr_t)frame + channel->page * sizeof(rgb_frame_page_t) == 0u) {
        return 0;
    }

    *page_r = channel->page;
    *page_g = channel->page;
    *page_b = channel->page;

    pwm = &frame->pages[channel->page].pwm[channel->offset];
    *ch_r = pwm;
    *ch_g = pwm + 1;
    *ch_b = pwm + 2;
    return 1;
}

/*
 * Queues a frame page for transmission to the LED driver: stores the page number in the first
 * free entry of g_rgb_dirty_pages unless it is already queued. Interrupts are masked meanwhile.
 */
void rgb_mark_page_dirty(uint32_t page)
{
    uint32_t irq_state = hal_irq_save();

    for (int i = 0; i != RGB_DIRTY_PAGE_QUEUE_LEN; ++i) {
        int32_t entry = g_rgb_dirty_pages[i];

        if (entry < 0 || (uint32_t)entry == page) {
            g_rgb_dirty_pages[i] = (int8_t)page;
            break;
        }
    }
    hal_irq_restore(irq_state);
}

/* Queues a page of g_rgb_frame_main unless its number is not a frame page. */
static void rgb_frame_mark_led_page(uint8_t page)
{
    if (page <= RGB_FRAME_PAGE_COUNT - 1) {
        rgb_mark_page_dirty(page);
    }
}

/*
 * Common tail of rgb_frame_set_led and rgb_frame_scale_led. Stores the three channel values
 * that differ from the frame content; for g_rgb_frame_main every page with a changed byte is
 * queued once.
 */
static void rgb_frame_store_led(rgb_frame_t *frame, uint8_t page_r, uint8_t page_g, uint8_t page_b,
                                uint8_t *ch_r, uint8_t *ch_g, uint8_t *ch_b, uint8_t r, uint8_t g, uint8_t b)
{
    bool track = (frame == &g_rgb_frame_main);
    bool r_changed = false;
    bool g_changed = false;

    if (*ch_r != r) {
        *ch_r = r;
        r_changed = true;
        if (track) {
            rgb_frame_mark_led_page(page_r);
        }
    }
    if (*ch_g != g) {
        *ch_g = g;
        g_changed = true;
        if (track && !(r_changed && page_g == page_r)) {
            rgb_frame_mark_led_page(page_g);
        }
    }
    if (*ch_b != b) {
        *ch_b = b;
        if (track && !(r_changed && page_b == page_r) && !(g_changed && page_b == page_g)) {
            rgb_frame_mark_led_page(page_b);
        }
    }
}

/* Sets the colour of one LED of a frame; LED numbers without a frame position are ignored. */
void rgb_frame_set_led(rgb_frame_t *frame, uint32_t led_id, uint8_t r, uint8_t g, uint8_t b)
{
    uint8_t page_r;
    uint8_t page_g;
    uint8_t page_b;
    uint8_t *ch_r;
    uint8_t *ch_g;
    uint8_t *ch_b;

    if (rgb_frame_locate_led(frame, led_id, &page_r, &page_g, &page_b, &ch_r, &ch_g, &ch_b) == 0) {
        return;
    }

    /* The frame stores (gain * (v + 1)) >> 8 per channel: with a gain of 255, 255 stays 255 and every
     * other value loses one step at most. */
    rgb_frame_store_led(frame, page_r, page_g, page_b, ch_r, ch_g, ch_b,
                        (uint8_t)((uint32_t)RGB_LED_GAIN_RED * (r + 1u) >> 8),
                        (uint8_t)((uint32_t)RGB_LED_GAIN_GREEN * (g + 1u) >> 8),
                        (uint8_t)((uint32_t)RGB_LED_GAIN_BLUE * (b + 1u) >> 8));
}

/* Sets the LED of the key at (row, col) to the colour (r, g, b) dimmed by scale / 256 (255 = undimmed). */
void rgb_frame_set_key_scaled(rgb_frame_t *frame, uint32_t row, uint32_t col, uint8_t r, uint8_t g, uint8_t b,
                              uint8_t scale)
{
    if (scale != 255) {
        b = (uint8_t)((uint16_t)(scale + b * scale) >> 8);
        g = (uint8_t)((uint16_t)(scale + g * scale) >> 8);
        r = (uint8_t)((uint16_t)(scale + r * scale) >> 8);
    }
    rgb_frame_set_led(frame, g_rgb_key_to_led[col + RGB_MATRIX_COLS * row], r, g, b);
}

/* Sets the LEDs of all key matrix positions to one colour. */
void rgb_frame_fill_keys(rgb_frame_t *frame, uint8_t r, uint8_t g, uint8_t b)
{
    for (int row = 0; row != RGB_MATRIX_ROWS; ++row) {
        for (int col = 0; col != RGB_MATRIX_COLS; ++col) {
            rgb_frame_set_led(frame, g_rgb_key_to_led[row * RGB_MATRIX_COLS + col], r, g, b);
        }
    }
}

/* Dims the colour an LED already has in the frame: channel = (scale + channel * scale) >> 8. */
void rgb_frame_scale_led(rgb_frame_t *frame, uint32_t led_id, uint32_t scale)
{
    uint16_t factor = (uint16_t)scale;
    uint8_t page_r;
    uint8_t page_g;
    uint8_t page_b;
    uint8_t *ch_r;
    uint8_t *ch_g;
    uint8_t *ch_b;

    if (rgb_frame_locate_led(frame, led_id, &page_r, &page_g, &page_b, &ch_r, &ch_g, &ch_b) == 0) {
        return;
    }

    rgb_frame_store_led(frame, page_r, page_g, page_b, ch_r, ch_g, ch_b,
                        (uint8_t)((uint16_t)(factor + *ch_r * factor) >> 8),
                        (uint8_t)((uint16_t)(factor + *ch_g * factor) >> 8),
                        (uint8_t)((uint16_t)(factor + *ch_b * factor) >> 8));
}

/*
 * Requests a redraw of effects and light bar and, when the key LEDs show the base colours (or
 * force != 0), paints the base colour map into g_rgb_frame_main. Then applies the brightness.
 */
void rgb_paint_base_colors(uint32_t force)
{
    g_rgb_effect_redraw_request = 1;
    lightbar_request_redraw();

    /*
     * The key LEDs show something else while the host drives them (SDK mode), while effect
     * layers are rendered (scan mode level 0 and at least one layer) or while a layer colour
     * map is shown; then only a forced call repaints.
     */
    if (force != 0
        || !(g_rgb_sdk_mode_active != 0
             || ((g_run_state_flags >> RUN_STATE_SCAN_MODE_SHIFT) == 0 && g_rgb_effect_layer_count != 0)
             || g_rgb_shown_color_layer != 0)) {
        for (int row = 0; row != RGB_MATRIX_ROWS; ++row) {
            for (int col = 0; col != RGB_MATRIX_COLS; ++col) {
                uint32_t led_id = g_rgb_key_to_led[row * RGB_MATRIX_COLS + col];
                uint32_t argb = rgb_convert_rgb565_to_argb(g_rgb_base_colors[row][col]);

                rgb_frame_set_led(&g_rgb_frame_main, led_id, (uint8_t)(argb >> 16), (uint8_t)(argb >> 8),
                                  (uint8_t)argb);
            }
        }
    }
    rgb_apply_brightness_change();
}
