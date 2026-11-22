/*
 * lightbar_data.c - definitions of the RAM objects of feature `lightbar` (declarations and
 * descriptions: lightbar_internal.h).
 */
#include "rgb/lightbar/lightbar_internal.h"

LightbarConfig g_rgb_lightbar_config;
GlobalLightbarConfig g_rgb_global_lightbar_config;

rgb_lightbar_state_t g_rgb_lightbar_states[RGB_LIGHTBAR_ENTRY_MAX];
rgb_lightbar_state_t g_rgb_global_lightbar_states[RGB_LIGHTBAR_ENTRY_MAX];

uint8_t g_rgb_lightbar_redraw;
uint8_t g_rgb_lightbar_active;
uint8_t g_rgb_lightbar_brightness;

lightbar_page_channels_t g_lightbar_page_channels[RGB_FRAME_PAGE_COUNT];

/* Sizes match stock firmware 2.14.1. */
_Static_assert(sizeof(g_rgb_lightbar_config) == 828, "g_rgb_lightbar_config");
_Static_assert(sizeof(g_rgb_global_lightbar_config) == 820, "g_rgb_global_lightbar_config");
_Static_assert(sizeof(g_rgb_global_lightbar_states) == 368, "g_rgb_global_lightbar_states");
_Static_assert(sizeof(g_rgb_lightbar_states) == 368, "g_rgb_lightbar_states");
