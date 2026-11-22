/*
 * lightbar_defaults.c - the writers of the factory light-bar files of the first boot: file `lb`
 * of a built-in profile and the root file `glb`, each from the factory content that the board's
 * defaults give for it (lightbar_profile_default_t, lightbar_global_default_t). The caller has
 * checked that the file is missing.
 *
 * Depends on the default-file helpers of the profile module (profile/profile_defaults.h).
 */
#include <string.h>

#include "profile/profile_defaults.h"
#include "rgb/lightbar/lightbar_internal.h"

/* `lb`: the light bar on, showing the entries of `glb` at the brightness of the profile. */
void lightbar_write_default_profile_file(uint32_t profile_index, const lightbar_profile_default_t *defaults,
                                         bool *created)
{
    LightbarConfig lightbar;

    if (!defaults->has_file) {
        return;
    }
    memset(&lightbar, 0, sizeof(lightbar));
    lightbar.enabled = true;
    lightbar.use_global = true;
    lightbar.has_brightness = true;
    lightbar.brightness = defaults->brightness;
    profile_default_write_pb_file(profile_index, "lb", &LightbarConfig_msg, LightbarConfig_size, &lightbar);
    *created = true;
}

/* `glb`: one profile indicator over every light-bar position. */
void lightbar_write_default_global_file(const lightbar_global_default_t *defaults)
{
    GlobalLightbarConfig global_lightbar;

    memset(&global_lightbar, 0, sizeof(global_lightbar));
    global_lightbar.entries_count = 1u;

    LightbarEntry *entry = &global_lightbar.entries[0];
    LightbarProfileIndicator *indicator = &entry->effect.profile_indicator;

    entry->which_effect = LightbarEntry_profile_indicator_tag;
    indicator->which_target = LightbarProfileIndicator_range_tag;
    indicator->target.range.first = 0u;
    indicator->target.range.last = LIGHTBAR_LED_COUNT - 1u;
    indicator->background_mode = defaults->background_mode;
    if (defaults->segment_count != 0u) {
        indicator->has_segment_count = true;
        indicator->segment_count = defaults->segment_count;
    }
    profile_default_write_root_pb_file("glb", &GlobalLightbarConfig_msg, GlobalLightbarConfig_size,
                                       &global_lightbar);
}
