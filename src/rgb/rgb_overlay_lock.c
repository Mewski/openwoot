/*
 * rgb_overlay_lock.c - lock indicators (Caps Lock, Scroll Lock, Num Lock, Fn-layer lock):
 * each one is a solid single-key overlay on the key that carries the function.
 */
#include "rgb/rgb_frame.h"

#include <string.h>

#include "layout/layout_data.h"
#include "profile/profile_data.h"
#include "tables/rgb_color_tables.h"
#include "usb/usb_data.h"
#include "usb/usb_types.h"

#define LAYOUT_LAYER_STATE_LOCK_MASK 0x30 /* g_layout_layer_state bits 5:4: a layer is locked */

/* Fills a solid-key overlay record with a packed colour of the rgb settings. */
static void rgb_lock_indicator_params(rgb_overlay_params_t *params, uint32_t packed_color, uint8_t key_pos)
{
    uint32_t argb;

    memset(params, 0, sizeof(*params)); /* type 0 = RGB_OVERLAY_SOLID_KEY */
    argb = rgb_unpack_color(packed_color);
    params->u.solid_key.color.b = (uint8_t)argb;
    params->u.solid_key.color.g = (uint8_t)(argb >> 8);
    params->u.solid_key.color.r = (uint8_t)(argb >> 16);
    params->u.solid_key.color.a = (uint8_t)(argb >> 24);
    params->u.solid_key.key_pos = key_pos;
}

/*
 * Switches one lock indicator (enum rgb_lock_indicator) on or off: starts or stops the solid
 * overlay on the key that carries the function, in the indicator colour of the rgb settings.
 */
void rgb_set_lock_indicator(uint32_t indicator, uint32_t on)
{
    rgb_overlay_params_t params;
    uint32_t overlay_id;
    uint32_t packed_color;
    uint8_t key_pos;

    if (indicator == RGB_INDICATOR_LAYER_LOCK) {
        key_pos = g_rgb_layer_lock_key_pos;
        if (on == 0 || key_pos == RGB_KEY_POS_NONE) {
            rgb_overlay_stop(RGB_OVERLAY_ID_LAYER_LOCK);
            return;
        }
        rgb_lock_indicator_params(&params, g_rgb_settings.base.layer_lock_color, key_pos);
        overlay_id = RGB_OVERLAY_ID_LAYER_LOCK;
    } else {
        overlay_id = k_rgb_lock_indicator_overlay_ids[indicator];
        /* The host owns the key LEDs in SDK mode: the three host lock indicators are hidden. */
        if (g_rgb_sdk_mode_active != 0 || on == 0) {
            rgb_overlay_stop(overlay_id);
            return;
        }

        if (indicator == RGB_INDICATOR_NUM_LOCK) {
            key_pos = g_rgb_num_lock_key_pos;
            packed_color = g_rgb_settings.base.num_lock_color;
        } else if (indicator == RGB_INDICATOR_SCROLL_LOCK) {
            key_pos = g_rgb_scroll_lock_key_pos;
            packed_color = g_rgb_settings.base.scroll_lock_color;
        } else {
            key_pos = g_rgb_caps_lock_key_pos;
            packed_color = g_rgb_settings.base.caps_lock_color;
        }
        if (key_pos == RGB_KEY_POS_NONE) {
            rgb_overlay_stop(overlay_id);
            return;
        }
        rgb_lock_indicator_params(&params, packed_color, key_pos);
        if (overlay_id == RGB_OVERLAY_ID_NONE) {
            return;
        }
    }

    if (g_rgb_sdk_mode_active == 0) {
        rgb_overlay_start(overlay_id, RGB_OVERLAY_DURATION_FOREVER, &params);
    }
}

/* Sets all four lock indicators from the layer lock state and the host LED state. */
void rgb_update_lock_indicators(void)
{
    uint32_t num_lock_on;

    rgb_set_lock_indicator(RGB_INDICATOR_LAYER_LOCK, (g_layout_layer_state & LAYOUT_LAYER_STATE_LOCK_MASK) != 0);
    rgb_set_lock_indicator(RGB_INDICATOR_CAPS_LOCK, (g_usb_link_flags & USB_LINK_CAPS_LOCK) != 0);
    rgb_set_lock_indicator(RGB_INDICATOR_SCROLL_LOCK, (g_usb_link_flags & USB_LINK_SCROLL_LOCK) != 0);

    /* The Num Lock indicator is inverted (lit while Num Lock is off) unless the setting is present and false. */
    num_lock_on = (g_usb_link_flags & USB_LINK_NUM_LOCK) != 0;
    if (!g_profile_global_settings.has_num_lock_indicator_inverted
        || g_profile_global_settings.num_lock_indicator_inverted) {
        num_lock_on = (g_usb_link_flags & USB_LINK_NUM_LOCK) == 0;
    }
    rgb_set_lock_indicator(RGB_INDICATOR_NUM_LOCK, num_lock_on);
}
