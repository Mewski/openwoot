/*
 * profile_init.c - the start-up steps of the profile module: the number of stored profiles
 * of each namespace and the keyboard-wide settings (file "global").
 *
 * Depends on: core/startup.h, the storage module (directory scan, files), feature signals
 * (profile count).
 */
#include <string.h>

#include "core/startup.h"
#include "profile/profile.h"
#include "rgb/signals/signals.h"
#include "storage/storage.h"

/*
 * Counts the profile directories of both namespaces ("<n>" and "lp/<n>"); a count is
 * kept at its .data default when the directory scan fails. The number of built-in
 * profiles is published as signal RGB_SIGNAL_PROFILE_COUNT.
 */
static void profile_counts_init(void)
{
    uint8_t count;

    count = 0;
    if (storage_count_profile_dirs(PROFILE_NS_BUILTIN, &count) == STORAGE_OK) {
        g_profile_counts[PROFILE_COUNTS_BUILTIN] = count;
    }
    count = 0;
    if (storage_count_profile_dirs(PROFILE_NS_LP, &count) == STORAGE_OK) {
        g_profile_counts[PROFILE_COUNTS_LP] = count;
    }

    signals_publish_if_changed(RGB_SIGNAL_PROFILE_COUNT, (float)g_profile_counts[PROFILE_COUNTS_BUILTIN]);
}
STARTUP_STEP(STARTUP_PROFILE_COUNTS, profile_counts_init);

/*
 * File "global" -> g_profile_global_settings. Without a readable file the defaults
 * are: idle dimming after 5 minutes, LEDs off after 10 minutes with off_enabled = 1,
 * everything else zero. The has_ flags stay 0 too, so the idle timer of
 * rgb_update_frame stays off until the host writes the settings.
 */
static void global_settings_load(void)
{
    GlobalSettings defaults;

    if (g_storage_mounted != 0) {
        char path[] = "global";

        if (storage_read_pb_file(path, &GlobalSettings_msg, &g_profile_global_settings) == STORAGE_OK) {
            return;
        }
    }

    memset(&defaults, 0, sizeof(defaults));
    defaults.idle.dim_after_minutes = 5;
    defaults.idle.off_after_minutes = 10;
    defaults.idle.off_enabled = 1;
    g_profile_global_settings = defaults;
}
STARTUP_STEP(STARTUP_GLOBAL_SETTINGS, global_settings_load);
