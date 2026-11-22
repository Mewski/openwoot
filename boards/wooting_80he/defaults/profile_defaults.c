/*
 * Factory defaults of this board: the content of the factory profiles of a regular unit and of
 * a unit of the special edition ("TenZ", feature `edition`), the content of the light-bar files
 * (feature `lightbar`), and the order in which the first boot writes the default files. The
 * helpers that fill and write the files are in src/profile/profile_default_files.c; the colour
 * maps in tables/profile_tables.c.
 *
 * A regular unit gets two factory profiles (Typing, Rapid), an edition unit three (TenZ,
 * Typing, TenZ Ultra). The entries of the higher indices are the fallbacks of the loaders.
 */
#include <stddef.h>

#include "profile/profile.h"
#include "profile/profile_defaults.h"
#include "profile/edition/edition.h"

#include "keys/keys_types.h"
#include "storage/storage.h"
#include "tables/profile_tables.h"

/* ------------------------------------------------------------------------- */
/* Effect layers of file "rgb"                                               */
/* ------------------------------------------------------------------------- */

/* Rainbow wave under two reactive fades (white and yellow). */
static void profile_effects_typing(RgbSettings *rgb_settings)
{
    RgbEffectLayer *layers = rgb_settings->effect.layers.layers;

    rgb_settings->which_effect = RgbSettings_layers_tag;

    layers[0].which_effect = RgbEffectLayer_rainbow_wave_tag;
    layers[0].effect.rainbow_wave.hue_preset = RgbHuePreset_RGB_HUE_PRESET_113_TO_204;
    layers[0].effect.rainbow_wave.period_250ms = 34u;
    layers[0].effect.rainbow_wave.direction = 0u;

    layers[1].which_effect = RgbEffectLayer_reactive_fade_tag;
    layers[1].effect.reactive_fade.base_level = 0xFFu;
    layers[1].effect.reactive_fade.step_ms = 8u;
    layers[1].effect.reactive_fade.color = 0xFFFFu;
    layers[1].effect.reactive_fade.flags = 2u;

    layers[2].which_effect = RgbEffectLayer_reactive_fade_tag;
    rgb_settings->effect.layers.layers_count = 3u;
    layers[2].effect.reactive_fade.base_level = 0xFFu;
    layers[2].effect.reactive_fade.step_ms = 4u;
    layers[2].effect.reactive_fade.flags = 2u;
    layers[2].effect.reactive_fade.color = 0xFFE0u;
}

/* Red rainbow wave under the key test. */
static void profile_effects_rainbow_key_test(RgbSettings *rgb_settings)
{
    RgbEffectLayer *layers = rgb_settings->effect.layers.layers;

    rgb_settings->which_effect = RgbSettings_layers_tag;
    layers[0].which_effect = RgbEffectLayer_rainbow_wave_tag;
    layers[0].effect.rainbow_wave.direction = 0u;
    layers[0].effect.rainbow_wave.hue_preset = RgbHuePreset_RGB_HUE_PRESET_2_TO_39;
    layers[0].effect.rainbow_wave.period_250ms = 30u;
    rgb_settings->effect.layers.layers_count = 2u;
    layers[1].which_effect = RgbEffectLayer_key_test_tag;
    layers[1].effect.key_test.base_level = 0xFFu;
}

static void profile_effects_level_meter(RgbSettings *rgb_settings)
{
    RgbEffectLayer *layers = rgb_settings->effect.layers.layers;

    rgb_settings->which_effect = RgbSettings_layers_tag;
    rgb_settings->effect.layers.layers_count = 1u;
    layers[0].which_effect = RgbEffectLayer_level_meter_tag;
    layers[0].effect.level_meter.base_level = 51u;
    layers[0].effect.level_meter.use_gamepad_curve = false;
}

static void profile_effects_analog_glow(RgbSettings *rgb_settings)
{
    RgbEffectLayer *layers = rgb_settings->effect.layers.layers;

    rgb_settings->which_effect = RgbSettings_layers_tag;
    rgb_settings->effect.layers.layers_count = 1u;
    layers[0].which_effect = RgbEffectLayer_analog_glow_tag;
    layers[0].effect.analog_glow.base_level = 130u;
    layers[0].effect.analog_glow.use_gamepad_curve = false;
}

/* ------------------------------------------------------------------------- */
/* Factory profiles                                                          */
/* ------------------------------------------------------------------------- */

#define PROFILE_BRIGHTNESS_FULL     255u
#define PROFILE_BRIGHTNESS_EDITION  112u

/* Typing: actuation at about 1.0 mm of a 4.0 mm switch (v / 255), rapid trigger off. */
#define PROFILE_CFG_TYPING { .actuation_point = 63u, .rapid_trigger_non_continuous = true }
/* Rapid trigger: actuation and sensitivity at about 0.4 mm. */
#define PROFILE_CFG_RAPID \
    { .actuation_point = 26u, .rapid_trigger_sensitivity = 26u, .rapid_trigger_non_continuous = true }
/* Edition: rapid trigger over the whole travel with the typing actuation point. */
#define PROFILE_CFG_EDITION { .actuation_point = 63u, .rapid_trigger_sensitivity = 26u }

/* Light bar. A regular unit writes no `lb`; its `glb` shows one of five segments per profile on
 * a contrast background. An edition unit gives profiles 0 and 2 an `lb` with a brightness of
 * their own; its `glb` lights the profile's segment over a background of the profile colour. */
#define PROFILE_LIGHTBAR_BRIGHTNESS_DIM   127u
#define PROFILE_LIGHTBAR_BRIGHTNESS_FULL  255u
#define PROFILE_LIGHTBAR_SEGMENTS         5u

#define PROFILE_ACTUATION_SHALLOW   26u     /* movement keys */
#define PROFILE_ACTUATION_DEEP      191u    /* keys next to them, about 3.0 mm */

static const RgbColor k_profile_red = { 0xFFu, 0x28u, 0x28u };
static const RgbColor k_profile_blue = { 0x00u, 0x00u, 0xFFu };

/* Edition profiles 0 and 2: W, A, S, D and left Ctrl shallow, Esc, Tab, Caps Lock and Space deep. */
static const profile_default_actuation_t k_profile_edition_actuation_points[] = {
    { KEYS_KEY_ID(2, 2), PROFILE_ACTUATION_SHALLOW },
    { KEYS_KEY_ID(3, 1), PROFILE_ACTUATION_SHALLOW },
    { KEYS_KEY_ID(3, 2), PROFILE_ACTUATION_SHALLOW },
    { KEYS_KEY_ID(3, 3), PROFILE_ACTUATION_SHALLOW },
    { KEYS_KEY_ID(5, 0), PROFILE_ACTUATION_SHALLOW },
    { KEYS_KEY_ID(0, 0), PROFILE_ACTUATION_DEEP },
    { KEYS_KEY_ID(2, 0), PROFILE_ACTUATION_DEEP },
    { KEYS_KEY_ID(3, 0), PROFILE_ACTUATION_DEEP },
    { KEYS_KEY_ID(5, 6), PROFILE_ACTUATION_DEEP },
};

/* Edition profile 2: two depth-priority pairs, A with D and W with S. */
static const profile_default_key_pair_t k_profile_edition_key_pairs[] = {
    { KEYS_KEY_ID(3, 1), KEYS_KEY_ID(3, 3), TwoKeyResolution_TWO_KEY_RESOLUTION_BOTH },
    { KEYS_KEY_ID(2, 2), KEYS_KEY_ID(3, 2), TwoKeyResolution_TWO_KEY_RESOLUTION_BOTH },
};

#define PROFILE_TYPING                                                                              \
    {                                                                                               \
        .brightness = PROFILE_BRIGHTNESS_FULL, .fill_effects = profile_effects_typing,             \
        .base_colors = &k_profile_default_colors_a[0][0], .cfg = PROFILE_CFG_TYPING,                \
        .name = "Typing Profile", .color = &k_profile_blue, .icon = "woot:p:black",                 \
    }
#define PROFILE_ANALOG(effects)                                                                     \
    {                                                                                               \
        .brightness = PROFILE_BRIGHTNESS_FULL, .fill_effects = (effects), .cfg = PROFILE_CFG_RAPID, \
        .name = "Analog Profile",                                                                   \
    }
#define PROFILE_OTHER { .brightness = PROFILE_BRIGHTNESS_FULL, .cfg = PROFILE_CFG_TYPING }

/* Regular unit. */
static const profile_default_set_t k_profile_default_set = {
    .profiles = {
        PROFILE_TYPING,
        {
            .brightness = PROFILE_BRIGHTNESS_FULL,
            .fill_effects = profile_effects_rainbow_key_test,
            .base_colors = &k_profile_default_colors_b[0][0],
            .cfg = PROFILE_CFG_RAPID,
            .name = "Rapid Profile",
            .color = &k_profile_red,
            .icon = "switchy:laser",
        },
        PROFILE_ANALOG(profile_effects_level_meter),
        PROFILE_ANALOG(profile_effects_analog_glow),
    },
    .other = PROFILE_OTHER,
    .lightbar_global = {
        .background_mode = LightbarProfileBackground_LIGHTBAR_PROFILE_BACKGROUND_CONTRAST_COLOR,
        .segment_count = PROFILE_LIGHTBAR_SEGMENTS,
    },
};

/* Unit of the special edition (handed out by profile_edition_default_set). */
const profile_default_set_t k_profile_edition_default_set = {
    .profiles = {
        {
            .brightness = PROFILE_BRIGHTNESS_EDITION,
            .base_colors = &k_profile_default_colors_b[0][0],
            .cfg = PROFILE_CFG_EDITION,
            .actuation_points = k_profile_edition_actuation_points,
            .actuation_point_count = sizeof(k_profile_edition_actuation_points)
                                     / sizeof(k_profile_edition_actuation_points[0]),
            .name = "TenZ Profile",
            .color = &k_profile_red,
            .icon = "switchy:gamer",
            .lightbar = { .has_file = true, .brightness = PROFILE_LIGHTBAR_BRIGHTNESS_DIM },
        },
        PROFILE_TYPING,
        {
            .brightness = PROFILE_BRIGHTNESS_EDITION,
            .base_colors = &k_profile_default_colors_b[0][0],
            .cfg = { .actuation_point = 63u, .rapid_trigger_sensitivity = 26u, .scan_mode_parallel = true },
            .actuation_points = k_profile_edition_actuation_points,
            .actuation_point_count = sizeof(k_profile_edition_actuation_points)
                                     / sizeof(k_profile_edition_actuation_points[0]),
            .key_pairs = k_profile_edition_key_pairs,
            .key_pair_count = sizeof(k_profile_edition_key_pairs) / sizeof(k_profile_edition_key_pairs[0]),
            .name = "TenZ Ultra Profile",
            .color = &k_profile_red,
            .icon = "switchy:gamerUltra",
            .lightbar = { .has_file = true, .brightness = PROFILE_LIGHTBAR_BRIGHTNESS_FULL },
        },
        PROFILE_ANALOG(profile_effects_analog_glow),
    },
    .other = PROFILE_OTHER,
    .profile0_alias_mask = 0xFDu,
    .lightbar_global = {
        .background_mode = LightbarProfileBackground_LIGHTBAR_PROFILE_BACKGROUND_PROFILE_COLOR,
    },
};

/* The two sets above list one entry per built-in profile slot. */
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
 * the factory profiles and the root files "global" and "glb". Existing files are kept. The
 * function stops after a profile that needed nothing.
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
        profile_default_create_cfg(set, profile_index, &created);
        profile_default_create_actuation(set, profile_index, &created);
        profile_default_create_info(set, profile_index, created);
        profile_default_create_key_pairs(set, profile_index, &created);
        profile_default_create_lightbar(set, profile_index, &created);

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
        profile_default_create_lightbar_global(set);
    }
}
