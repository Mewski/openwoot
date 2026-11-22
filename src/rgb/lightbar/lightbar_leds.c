/*
 * lightbar_leds.c - the LEDs of the light bar: which LED number each light-bar position is
 * (BOARD_LIGHTBAR_LEDS of board.h; this table exists only on boards that list the feature),
 * where their channels sit in the frame pages, the light-bar brightness of those channels when
 * a page is sent, and their colours in the LED frame dump.
 *
 * Depends on core/startup.h, the frame layout of the board (tables/rgb_tables.h) and the
 * frame and output stage of rgb_frame.h.
 */
#include <string.h>

#include "rgb/lightbar/lightbar_internal.h"

#include "core/startup.h"
#include "rgb/rgb_frame.h"
#include "tables/rgb_color_tables.h"
#include "tables/rgb_tables.h"

enum {
    STARTUP_LIGHTBAR_LEDS = STARTUP_LED_BUS + 1, /* after the bus, before the driver */
};

const uint8_t k_lightbar_led_ids[LIGHTBAR_LED_COUNT] = { BOARD_LIGHTBAR_LEDS };

void lightbar_dump_led_frame(uint8_t *out)
{
    RgbColor last = { 0, 0, 0 };

    for (uint32_t i = 0; i != LIGHTBAR_LED_COUNT; i++) {
        uint16_t word = rgb_frame_read_led_rgb565(&g_rgb_frame_main, k_lightbar_led_ids[i], &last);

        memcpy(&out[i * sizeof(uint16_t)], &word, sizeof(word));
    }
}

/*
 * Appends the three PWM channels of one LED to the light-bar list of the frame page they live
 * in (k_rgb_led_channels). An LED number without a place in the frame is skipped.
 */
static void lightbar_register_led(uint8_t led_id)
{
    lightbar_page_channels_t *list;
    uint8_t offset;

    if (led_id >= RGB_LED_LIMIT || k_rgb_led_channels[led_id].page == RGB_LED_PAGE_NONE) {
        return;
    }

    offset = k_rgb_led_channels[led_id].offset;
    list = &g_lightbar_page_channels[k_rgb_led_channels[led_id].page];
    list->channels[list->count++] = offset;
    list->channels[list->count++] = (uint8_t)(offset + 1u);
    list->channels[list->count++] = (uint8_t)(offset + 2u);
}

/* Start-up step: records which pwm[] offsets of each frame page are light-bar channels. */
static void lightbar_leds_init(void)
{
    for (uint32_t i = 0; i < sizeof(k_lightbar_led_ids); i++) {
        lightbar_register_led(k_lightbar_led_ids[i]);
    }
}
STARTUP_STEP(STARTUP_LIGHTBAR_LEDS, lightbar_leds_init);

void lightbar_apply_page_brightness(const rgb_frame_page_t *page, rgb_frame_page_t *out, uint32_t fade_level)
{
    const lightbar_page_channels_t *list = &g_lightbar_page_channels[page->index];
    uint16_t scale = k_rgb_gamma_table[(uint32_t)g_rgb_lightbar_brightness * fade_level / 255];

    for (uint8_t i = 0; i < list->count; ++i) {
        uint8_t channel = list->channels[i];

        out->pwm[channel] = rgb_output_value(page->pwm[channel], scale);
    }
}
