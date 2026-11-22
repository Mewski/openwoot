/*
 * rgb_brightness.c - key brightness: propagation to the output stage, brightness keys
 * (software timer callbacks) and the delayed save of the rgb settings file.
 */
#include "rgb/rgb_frame.h"

#include "core/run_state.h"
#include "core/soft_timer.h"
#include "hal/hal_irq.h"
#include "profile/profile_data.h"
#include "rgb/lightbar/lightbar.h"
#include "rgb/signals/signals.h"
#include "storage/storage.h"
#include "storage/storage_data.h"

#define RGB_SETTINGS_FILE_NAME     "rgb"
#define RGB_SETTINGS_FILE_BUF_SIZE 182 /* encode buffer size for file `rgb` (largest encoded RgbSettings message) */
#define RGB_SAVE_DELAY_TICKS       80  /* x SOFT_TIMER_PERIOD_MS = 2 s after the last brightness step */

/* Soft timer callback: writes the rgb settings of the active profile to file `rgb` (only while the file system is mounted). */
void rgb_save_settings_timer_cb(void)
{
    if (g_storage_mounted != 0) {
        storage_write_profile_pb_file(g_profile_active_id.raw, RGB_SETTINGS_FILE_NAME, &RgbSettings_msg,
                                      RGB_SETTINGS_FILE_BUF_SIZE, &g_rgb_settings);
    }
}

/*
 * Takes over a changed settings brightness: stores it as the applied brightness and queues
 * all frame pages for a re-send (unless the frame is faded to black); passes it on to the
 * light bar when that has no brightness of its own.
 */
void rgb_apply_brightness_change(void)
{
    uint32_t brightness = g_rgb_settings.base.brightness;

    if (brightness != g_rgb_applied_brightness) {
        g_rgb_applied_brightness = (uint8_t)brightness;
        if (g_rgb_fade_level != 0) {
            for (int page = 0; page != RGB_FRAME_PAGE_COUNT; ++page) {
                rgb_mark_page_dirty((uint8_t)page);
            }
            brightness = g_rgb_settings.base.brightness;
        }
    }
    lightbar_apply_brightness_change(brightness);
}

/*
 * Brightness key: raises (increase != 0) or lowers the brightness by RGB_BRIGHTNESS_STEP within
 * 0..255 (0..179 while a fast scan mode is active), applies it, publishes it as signal
 * RGB_SIGNAL_BRIGHTNESS in percent and starts the 2 s timer that saves the settings.
 */
void rgb_step_brightness(uint32_t increase)
{
    uint32_t irq_state;

    if (increase != 0) {
        uint8_t max = RGB_BRIGHTNESS_MAX;

        if ((g_run_state_flags >> RUN_STATE_SCAN_MODE_SHIFT) != 0) {
            max = RGB_BRIGHTNESS_MAX_FAST_MODE;
        }
        if (g_rgb_settings.base.brightness >= max - (RGB_BRIGHTNESS_STEP - 1)) {
            g_rgb_settings.base.brightness = max;
        } else {
            g_rgb_settings.base.brightness += RGB_BRIGHTNESS_STEP;
        }
    } else if (g_rgb_settings.base.brightness <= RGB_BRIGHTNESS_STEP - 1) {
        g_rgb_settings.base.brightness = 0;
    } else {
        g_rgb_settings.base.brightness -= RGB_BRIGHTNESS_STEP;
    }

    rgb_apply_brightness_change();

    signals_publish_if_changed(RGB_SIGNAL_BRIGHTNESS, ((float)g_rgb_settings.base.brightness * 100.0f) / 255.0f);

    /* (Re)start the one-shot timer that saves the settings once the key repeats have stopped. */
    irq_state = hal_irq_save();
    g_soft_timer_expired_mask &= (uint8_t)~(1u << SOFT_TIMER_RGB_SAVE);
    g_soft_timer_active_mask |= 1u << SOFT_TIMER_RGB_SAVE;
    g_soft_timers[SOFT_TIMER_RGB_SAVE].remaining = RGB_SAVE_DELAY_TICKS;
    g_soft_timers[SOFT_TIMER_RGB_SAVE].reload = 0;
    hal_irq_restore(irq_state);
}

/* Soft timer callback of the "brightness up" key (key repeat). */
void rgb_brightness_up_timer_cb(void)
{
    rgb_step_brightness(1);
}

/* Soft timer callback of the "brightness down" key (key repeat). */
void rgb_brightness_down_timer_cb(void)
{
    rgb_step_brightness(0);
}
