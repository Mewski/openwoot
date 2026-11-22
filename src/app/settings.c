/*
 * app: the device settings at start-up. The packed device settings (file "ds" plus the
 * stored layout variant) are loaded into one block that lives here and is applied in two
 * steps: run flags and layout variant first, USB mode and gamepad polling rate after the
 * layout tables are selected. A last step latches the release rebound filter option of the
 * global settings.
 *
 * Depends on: core/run_state.h, core/startup.h, and the storage, layout, usb, profile and
 * analog modules whose state the settings set.
 */
#include <string.h>

#include "analog/analog_data.h"
#include "core/run_state.h"
#include "core/startup.h"
#include "layout/layout_data.h"
#include "layout/layout_types.h"
#include "profile/profile.h"
#include "storage/storage.h"
#include "usb/usb_data.h"
#include "usb/usb_types.h"

/* The device settings as loaded at start-up; settings_apply_usb_mode reads them again. */
static storage_device_settings_t settings_at_startup;

/*
 * Loads the packed device settings into the start-up block and applies the flag byte
 * and the layout variant.
 */
static void settings_load_and_apply(void)
{
    storage_device_settings_t *settings = &settings_at_startup;

    memset(settings, 0, sizeof(*settings));
    if (g_storage_mounted != 0) {
        storage_load_device_settings(settings);
    }

    g_usb_link_flags &= (uint8_t)~USB_LINK_CAPS_LOCK;

    /* Legacy flag (the loader never sets it): convert to USB mode 1 and save. */
    if ((settings->flags & STORAGE_DS_FLAG_LEGACY) != 0) {
        settings->flags &= (uint8_t)~STORAGE_DS_FLAG_LEGACY;
        settings->usb_flags = (uint8_t)((settings->usb_flags & ~STORAGE_DS_USB_MODE_MASK)
                                        | (USB_MODE_GAMEPAD << STORAGE_DS_USB_MODE_SHIFT));
        storage_save_device_settings(settings);
    }

    if ((settings->flags & STORAGE_DS_FLAG_6KRO) != 0) {
        g_run_state_flags |= RUN_STATE_FLAG_6KRO | RUN_STATE_FLAG_6KRO_ACTIVE;
    }
    if ((settings->flags & STORAGE_DS_FLAG_USB_FULL_SPEED) != 0) {
        g_run_state_options |= RUN_STATE_OPTION_USB_FULL_SPEED;
    }
    if (settings->layout_variant < BOARD_LAYOUT_VARIANT_COUNT) {
        g_layout_physical_variant = settings->layout_variant;
    }
}
STARTUP_STEP(STARTUP_SETTINGS, settings_load_and_apply);

/*
 * USB mode (usb_flags bits 2:1) -> g_usb_link_flags bits 1:0 (interface 0 variant, added
 * to the product id); the unused mode 3 is rewritten to 2 and saved. Gamepad polling rate
 * (bits 4:3) -> g_usb_gamepad_poll_rate bits 1:0, 3 taken as 1.
 */
static void settings_apply_usb_mode(void)
{
    storage_device_settings_t *settings = &settings_at_startup;
    uint8_t usb_flags = settings->usb_flags;
    uint8_t mode;
    uint8_t poll_rate;

    if ((usb_flags & STORAGE_DS_USB_MODE_MASK) == STORAGE_DS_USB_MODE_MASK) {
        settings->usb_flags = (uint8_t)((usb_flags & ~STORAGE_DS_USB_MODE_MASK)
                                        | (USB_MODE_NO_GAMEPAD << STORAGE_DS_USB_MODE_SHIFT));
        storage_save_device_settings(settings);
        usb_flags = settings->usb_flags;
    }

    mode = (usb_flags & STORAGE_DS_USB_MODE_MASK) >> STORAGE_DS_USB_MODE_SHIFT;
    poll_rate = (usb_flags & STORAGE_DS_USB_POLL_MASK) >> STORAGE_DS_USB_POLL_SHIFT;
    if (poll_rate == 3) {
        poll_rate = USB_GAMEPAD_POLL_1MS;
    }

    g_usb_link_flags = (uint8_t)((g_usb_link_flags & ~USB_LINK_MODE_MASK) | mode);
    g_usb_gamepad_poll_rate = (uint8_t)((g_usb_gamepad_poll_rate & ~USB_GAMEPAD_POLL_RATE_MASK) | poll_rate);
}
STARTUP_STEP(STARTUP_USB_MODE, settings_apply_usb_mode);

/*
 * Latches the "release rebound filter" option of the global settings (present and
 * true). When it turns on, the filter state of every key is reset. The same code is
 * in protocol_cmd_set_global_settings.
 */
static void rebound_filter_option_init(void)
{
    uint8_t enabled = 0;

    if (g_profile_global_settings.has_release_rebound_filter_enabled) {
        enabled = g_profile_global_settings.release_rebound_filter_enabled;
    }

    if (enabled != 0 && g_profile_global_option3_active == 0) {
        for (uint32_t pos = 0; pos < ANALOG_KEY_COUNT; pos++) {
            analog_rebound_filter_t *filter = &g_analog_key_states[pos].rebound;

            filter->prev_reading = ANALOG_VALUE_NONE;
            filter->rest_timer_running = 0;
            filter->rest_timer_start_us = 0;
            filter->state = ANALOG_REBOUND_IDLE;
        }
    }
    g_profile_global_option3_active = enabled;
}
STARTUP_STEP(STARTUP_REBOUND_FILTER, rebound_filter_option_init);
