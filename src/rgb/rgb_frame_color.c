/*
 * rgb_frame_color.c - colour conversions, and the colour of one LED of a frame read back as
 * RGB565. Colours travel as 0xAARRGGBB in a uint32_t (byte 0 = blue, the layout of rgb_argb_t).
 */
#include "rgb/rgb_frame.h"

#define RGB_ARGB(a, r, g, b) \
    ((uint32_t)(b) | ((uint32_t)(g) << 8) | ((uint32_t)(r) << 16) | ((uint32_t)(a) << 24))

/* Blends two colours: t = 0 gives `from`, t = 255 gives `to`; the result has alpha 0xFF otherwise. */
uint32_t rgb_blend_argb(uint32_t from, uint32_t to, uint32_t t)
{
    uint8_t r;
    uint8_t g;
    uint8_t b;

    if (t == 0) {
        return from;
    }
    if (t == 255) {
        return to;
    }

    /* 16-bit arithmetic: (to * t + from * (255 - t)) >> 8 per channel, alpha forced to 0xFF. */
    r = (uint8_t)((uint16_t)((uint8_t)(to >> 16) * t + (255 - t) * (uint8_t)(from >> 16)) >> 8);
    b = (uint8_t)((uint16_t)((uint8_t)to * t + (255 - t) * (uint8_t)from) >> 8);
    g = (uint8_t)((uint16_t)((uint8_t)(to >> 8) * t + (255 - t) * (uint8_t)(from >> 8)) >> 8);
    return RGB_ARGB(0xFF, r, g, b);
}

/* Expands an RGB565 colour to 0xFFRRGGBB. */
uint32_t rgb_convert_rgb565_to_argb(uint16_t rgb565)
{
    uint8_t r = (uint8_t)(rgb565 >> 8) & 0xF8;
    uint8_t g = (uint8_t)(rgb565 >> 3) & 0xFC;
    uint8_t b = (uint8_t)(rgb565 << 3);

    /* Non-zero channels get their low bits filled with ones, so the maximum becomes 255. */
    if (r != 0) {
        r += 7;
    }
    if (g != 0) {
        g += 3;
    }
    if (b != 0) {
        b += 7;
    }
    return RGB_ARGB(0xFF, r, g, b);
}

/* Decodes a packed colour parameter (see RGB_PACKED_COLOR_RGB888 in rgb_types.h) to 0xAARRGGBB. */
uint32_t rgb_unpack_color(uint32_t packed)
{
    if ((packed & RGB_PACKED_COLOR_RGB888) != 0) {
        /* RGB888 in bits 23:0, 7-bit alpha in bits 30:24. */
        return (packed & 0x00FFFFFFu) | ((uint32_t)RGB_PACKED_COLOR_ALPHA(packed) << 24);
    }
    /* RGB565 in bits 15:0. */
    return rgb_convert_rgb565_to_argb((uint16_t)packed);
}

uint16_t rgb_frame_read_led_rgb565(rgb_frame_t *frame, uint32_t led_id, RgbColor *last)
{
    uint8_t page_r;
    uint8_t page_g;
    uint8_t page_b;
    uint8_t *ch_r;
    uint8_t *ch_g;
    uint8_t *ch_b;

    if (rgb_frame_locate_led(frame, led_id, &page_r, &page_g, &page_b, &ch_r, &ch_g, &ch_b) != 0) {
        last->r = *ch_r;
        last->g = *ch_g;
        last->b = *ch_b;
    }
    return (uint16_t)(((last->r << 8) & 0xF800) | ((last->g << 3) & 0x07E0) | (last->b >> 3));
}
