/*
 * Factory defaults of this board: the content of the factory profiles and the order in which the
 * first boot writes the default files. The helpers that fill and write the files are in
 * src/profile/profile_default_files.c; the colour maps in tables/profile_tables.c.
 *
 * Every unit gets two factory profiles (Typing, Rapid); the entries of indices 2 and 3 are only
 * the fallbacks of the loaders (no effect, rapid trigger cfg). Beside the files of the other
 * boards, the first boot writes the colours of layer 2 ("r2") and the third default key map
 * layer ("ml2"). The board has no light bar, so it has no light-bar content and writes no "lb"
 * or "glb".
 */
#include <stddef.h>

#include "profile/profile.h"
#include "profile/profile_defaults.h"
#include "profile/edition/edition.h"

#include "storage/storage.h"
#include "tables/profile_tables.h"

/* ------------------------------------------------------------------------- */
/* Effect layers of file "rgb"                                               */
/* ------------------------------------------------------------------------- */

/* Full-circle rainbow wave under a white reactive fade. */
static void profile_effects_typing(RgbSettings *rgb_settings)
{
    RgbEffectLayer *layers = rgb_settings->effect.layers.layers;

    rgb_settings->which_effect = RgbSettings_layers_tag;
    rgb_settings->effect.layers.layers_count = 2u;

    layers[0].which_effect = RgbEffectLayer_rainbow_wave_tag;
    layers[0].effect.rainbow_wave.hue_preset = RgbHuePreset_RGB_HUE_PRESET_FULL_CIRCLE;
    layers[0].effect.rainbow_wave.period_250ms = 65u;
    layers[0].effect.rainbow_wave.direction = 0u;

    layers[1].which_effect = RgbEffectLayer_reactive_fade_tag;
    layers[1].effect.reactive_fade.base_level = 0xFFu;
    layers[1].effect.reactive_fade.step_ms = 6u;
    layers[1].effect.reactive_fade.flags = 2u;
    layers[1].effect.reactive_fade.color = 0xFFFFu;
}

/* Red rainbow wave under the key test. */
static void profile_effects_rainbow_key_test(RgbSettings *rgb_settings)
{
    RgbEffectLayer *layers = rgb_settings->effect.layers.layers;

    rgb_settings->which_effect = RgbSettings_layers_tag;
    rgb_settings->effect.layers.layers_count = 2u;

    layers[0].which_effect = RgbEffectLayer_rainbow_wave_tag;
    layers[0].effect.rainbow_wave.hue_preset = RgbHuePreset_RGB_HUE_PRESET_2_TO_39;
    layers[0].effect.rainbow_wave.period_250ms = 30u;
    layers[0].effect.rainbow_wave.direction = 0u;

    layers[1].which_effect = RgbEffectLayer_key_test_tag;
    layers[1].effect.key_test.base_level = 0xFFu;
}

/* ------------------------------------------------------------------------- */
/* Factory profiles                                                          */
/* ------------------------------------------------------------------------- */

#define PROFILE_BRIGHTNESS_FULL     255u

/* Rapid trigger with a separate release sensitivity. */
#define PROFILE_CFG_RAPID                                                                           \
    {                                                                                               \
        .actuation_point = 26u, .rapid_trigger_sensitivity = 13u,                                   \
        .rapid_trigger_release_sensitivity = 35u, .rapid_trigger_non_continuous = true,             \
    }
#define PROFILE_UNCREATED { .brightness = PROFILE_BRIGHTNESS_FULL, .cfg = PROFILE_CFG_RAPID }

static const RgbColor k_profile_red = { 0xFFu, 0x28u, 0x28u };
static const RgbColor k_profile_blue = { 0x00u, 0x00u, 0xFFu };

static const profile_default_set_t k_profile_default_set = {
    .profiles = {
        {
            .brightness = PROFILE_BRIGHTNESS_FULL,
            .fill_effects = profile_effects_typing,
            .base_colors = &k_profile_default_colors_a[0][0],
            .cfg = { .actuation_point = 102u, .rapid_trigger_non_continuous = true },
            .name = "Typing Profile",
            .color = &k_profile_blue,
            .icon = "woot:p:black",
        },
        {
            .brightness = PROFILE_BRIGHTNESS_FULL,
            .fill_effects = profile_effects_rainbow_key_test,
            .base_colors = &k_profile_default_colors_b[0][0],
            .cfg = PROFILE_CFG_RAPID,
            .name = "Rapid Profile",
            .color = &k_profile_red,
            .icon = "switchy:laser",
        },
        PROFILE_UNCREATED,
        PROFILE_UNCREATED,
    },
    .other = {
        .brightness = PROFILE_BRIGHTNESS_FULL,
        .cfg = { .actuation_point = 63u, .rapid_trigger_non_continuous = true },
    },
};

/* The set above lists one entry per built-in profile slot. */
_Static_assert(PROFILE_BUILTIN_MAX == 4u, "factory profile tables have four entries");

/* ------------------------------------------------------------------------- */
/* Functions of profile/profile.h                                            */
/* ------------------------------------------------------------------------- */

void profile_fill_default_rgb_settings(uint32_t profile_index, RgbSettings *rgb_settings)
{
    profile_default_fill_rgb(&k_profile_default_set, profile_index, rgb_settings);
}

void profile_copy_default_base_colors(uint32_t profile_index, uint16_t *colors)
{
    profile_default_copy_base_colors(&k_profile_default_set, profile_index, colors);
}

void profile_fill_default_cfg(uint32_t profile_index, ProfileConfig *cfg)
{
    profile_default_fill_cfg(&k_profile_default_set, profile_index, cfg);
}

/*
 * First boot (after the profile directories were made): writes every missing default file of
 * the factory profiles and the root file "global". Existing files are kept. The function stops
 * after a profile that needed nothing. The device settings are loaded as on the other boards;
 * this board has no key map patch of a physical variant.
 */
void profile_create_factory_defaults(void)
{
    const profile_default_set_t *set = &k_profile_default_set;
    storage_device_settings_t settings = { 0 };
    bool created = false;

    if (g_storage_mounted != 0u) {
        storage_load_device_settings(&settings);
    }
    uint32_t profile_count = profile_edition_factory_profile_count();

    for (uint32_t profile_index = 0u;; profile_index++) {
        profile_default_create_rgb(set, profile_index, &created);
        profile_default_create_base_colors(set, profile_index, &created);
        profile_default_create_layer_colors(profile_index, 1u, &k_profile_default_layer1_colors[0][0], &created);
        profile_default_create_layer_colors(profile_index, 2u, &k_profile_default_layer2_colors[0][0], &created);
        profile_default_create_cfg(set, profile_index, &created);
        profile_default_create_info(set, profile_index, created);

        if (profile_index == 0u) {
            profile_default_create_keymaps(settings.layout_variant, &created);
        } else {
            /* Both files are probed, nothing is written. Matches stock firmware 2.14.1. */
            (void)profile_default_file_missing(profile_index, "gm");
            (void)profile_default_file_missing(profile_index, "gp");
            if (profile_count <= profile_index + 1u) {
                break;
            }
        }
        if (!created) {
            return;
        }
    }

    if (created) {
        profile_default_create_global();
    }
}
