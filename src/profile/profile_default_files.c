/*
 * profile_default_files.c - the parts of the factory defaults that every board shares: the
 * factory profile of an index in a board's data set, the loader fallbacks built from it, and the
 * default files of the first boot, each written only if it is missing. The board's
 * defaults/profile_defaults.c holds the data and calls these in its order. The files of a
 * feature are tested for here and written by the feature from its part of the data.
 *
 * Depends on: the storage module (files), feature `edition` (data set of a special edition),
 * feature `lightbar` (writers of its files), the layout tables of the board (default key map
 * layers, physical variants).
 */
#include <stdio.h>
#include <string.h>

#include <pb_encode.h>

#include "profile/profile.h"
#include "profile/profile_defaults.h"
#include "profile/edition/edition.h"

#include "layout/layout_types.h"
#include "storage/storage.h"
#include "tables/layout_tables.h"

/* Encode buffer sizes of the default files. */
#define PROFILE_RGB_ENCODED_MAX     182u
#define PROFILE_CFG_ENCODED_MAX     26u
#define PROFILE_ACT_ENCODED_MAX     519u
#define PROFILE_INFO_ENCODED_MAX    65u
#define PROFILE_GLOBAL_ENCODED_MAX  56u

/* Number of default key map layers that are written also when nothing was created before. */
#define PROFILE_KEYMAPS_ALWAYS_WRITTEN 2u

/* The factory profile of index profile_index: that of the edition on a special unit. */
static const profile_default_t *profile_default_get(const profile_default_set_t *regular, uint32_t profile_index)
{
    const profile_default_set_t *set = profile_edition_default_set(regular);

    if (profile_index < PROFILE_BUILTIN_MAX) {
        return &set->profiles[profile_index];
    }
    if (set->profile0_alias_mask != 0u && (profile_index & set->profile0_alias_mask) == 0u) {
        return &set->profiles[0];
    }
    return &set->other;
}

/* ------------------------------------------------------------------------- */
/* Loader fallbacks                                                          */
/* ------------------------------------------------------------------------- */

void profile_default_fill_rgb(const profile_default_set_t *regular, uint32_t profile_index,
                              RgbSettings *rgb_settings)
{
    rgb_settings->base.caps_lock_color = 0xFFFFu;
    rgb_settings->base.layer_lock_color = 0xFFFFu;
    rgb_settings->which_effect = 0u;

    const profile_default_t *profile = profile_default_get(regular, profile_index);

    rgb_settings->base.brightness = profile->brightness;
    if (profile->fill_effects != NULL) {
        profile->fill_effects(rgb_settings);
    }
}

void profile_default_copy_base_colors(const profile_default_set_t *regular, uint32_t profile_index,
                                      uint16_t *colors)
{
    const profile_default_t *profile = profile_default_get(regular, profile_index);

    if (profile->base_colors != NULL) {
        memcpy(colors, profile->base_colors, STORAGE_BASE_COLORS_SIZE);
    }
}

void profile_default_fill_cfg(const profile_default_set_t *regular, uint32_t profile_index, ProfileConfig *cfg)
{
    const profile_default_cfg_t *values = &profile_default_get(regular, profile_index)->cfg;

    cfg->actuation_point = values->actuation_point;
    if (values->rapid_trigger_sensitivity != 0u) {
        cfg->rapid_trigger_enabled = true;
        cfg->has_rapid_trigger_sensitivity = true;
        cfg->rapid_trigger_sensitivity = values->rapid_trigger_sensitivity;
    }
    if (values->rapid_trigger_release_sensitivity != 0u) {
        cfg->has_rapid_trigger_release_sensitivity = true;
        cfg->rapid_trigger_release_sensitivity = values->rapid_trigger_release_sensitivity;
    }
    cfg->has_rapid_trigger_non_continuous = true;
    cfg->rapid_trigger_non_continuous = values->rapid_trigger_non_continuous;
    if (values->scan_mode_parallel) {
        cfg->scan_mode_enabled = true;
        cfg->has_scan_mode = true;
        cfg->scan_mode = ScanMode_SCAN_MODE_PARALLEL;
    }
}

/* ------------------------------------------------------------------------- */
/* First-boot files                                                          */
/* ------------------------------------------------------------------------- */

bool profile_default_file_missing(uint32_t profile_index, const char *name)
{
    char path[28];

    storage_format_profile_path(path, PROFILE_ID_RAW(profile_index, PROFILE_NS_BUILTIN), name);
    return storage_file_exists(path) == 0;
}

/* The same test for a numbered file ("r<k>", "ml<k>"). */
static bool profile_default_numbered_file_missing(uint32_t profile_index, const char *name_format, int number)
{
    char name[16];
    char dir[8];
    char path[28];

    sprintf(name, name_format, number);
    sprintf(dir, "%d", (int)profile_index);
    sprintf(path, "%s/%s", dir, name);
    return storage_file_exists(path) == 0;
}

void profile_default_write_pb_file(uint32_t profile_index, const char *name, const pb_msgdesc_t *msgdesc,
                                   uint32_t max_size, const void *msg)
{
    if (g_storage_mounted != 0u) {
        storage_write_profile_pb_file(PROFILE_ID_RAW(profile_index, PROFILE_NS_BUILTIN), name, msgdesc, max_size,
                                      msg);
    }
}

void profile_default_write_root_pb_file(const char *name, const pb_msgdesc_t *msgdesc, uint32_t max_size,
                                        const void *msg)
{
    if (g_storage_mounted == 0u) {
        return;
    }

    uint8_t encoded[max_size];
    pb_ostream_t stream = pb_ostream_from_buffer(encoded, max_size);
    pb_encode(&stream, msgdesc, msg); /* result ignored, matches stock firmware 2.14.1 */
    storage_write_file(name, encoded, (uint16_t)stream.bytes_written);
}

void profile_default_create_rgb(const profile_default_set_t *regular, uint32_t profile_index, bool *created)
{
    RgbSettings rgb_settings;

    if (!profile_default_file_missing(profile_index, "rgb")) {
        return;
    }
    memset(&rgb_settings, 0, sizeof(rgb_settings));
    rgb_settings.base = (RgbBaseSettings)RgbBaseSettings_init_default;
    profile_default_fill_rgb(regular, profile_index, &rgb_settings);
    profile_default_write_pb_file(profile_index, "rgb", &RgbSettings_msg, PROFILE_RGB_ENCODED_MAX, &rgb_settings);
    *created = true;
}

void profile_default_create_base_colors(const profile_default_set_t *regular, uint32_t profile_index,
                                        bool *created)
{
    uint16_t colors[STORAGE_BASE_COLORS_SIZE / sizeof(uint16_t)];

    if (!profile_default_file_missing(profile_index, "clr")) {
        return;
    }
    memset(colors, 0, sizeof(colors));
    profile_default_copy_base_colors(regular, profile_index, colors);
    storage_write_base_colors(PROFILE_ID_RAW(profile_index, PROFILE_NS_BUILTIN), colors);
    *created = true;
}

void profile_default_create_layer_colors(uint32_t profile_index, uint32_t layer, const uint16_t *colors,
                                         bool *created)
{
    uint16_t buffer[STORAGE_LAYER_COLORS_SIZE / sizeof(uint16_t)];

    if (!profile_default_numbered_file_missing(profile_index, "r%d", (int)layer) || !*created) {
        return;
    }
    memcpy(buffer, colors, sizeof(buffer));
    storage_write_layer_colors(PROFILE_ID_RAW(profile_index, PROFILE_NS_BUILTIN), layer, buffer);
    *created = true;
}

void profile_default_create_cfg(const profile_default_set_t *regular, uint32_t profile_index, bool *created)
{
    ProfileConfigFile file = { ProfileConfig_init_default };

    if (!profile_default_file_missing(profile_index, "cfg")) {
        return;
    }
    profile_default_fill_cfg(regular, profile_index, &file.config);
    profile_default_write_pb_file(profile_index, "cfg", &ProfileConfigFile_msg, PROFILE_CFG_ENCODED_MAX, &file);
    *created = true;
}

void profile_default_create_actuation(const profile_default_set_t *regular, uint32_t profile_index,
                                      bool *created)
{
    ActuationPoints act;

    if (!profile_default_file_missing(profile_index, "act")) {
        return;
    }
    memset(&act, 0, sizeof(act));

    const profile_default_t *profile = profile_default_get(regular, profile_index);

    if (profile->actuation_point_count == 0u) {
        return;
    }
    act.which_map = ActuationPoints_list_u8_tag;
    for (uint32_t i = 0u; i < profile->actuation_point_count; i++) {
        const profile_default_actuation_t *entry = &profile->actuation_points[i];

        /* low byte key id, high byte actuation point */
        act.map.list_u8.items[i] = (uint16_t)(entry->key_id | ((uint16_t)entry->actuation_point << 8));
    }
    act.map.list_u8.items_count = profile->actuation_point_count;
    profile_default_write_pb_file(profile_index, "act", &ActuationPoints_msg, PROFILE_ACT_ENCODED_MAX, &act);
    *created = true;
}

void profile_default_create_info(const profile_default_set_t *regular, uint32_t profile_index, bool created)
{
    ProfileInfo info;

    if (!profile_default_file_missing(profile_index, "i") || !created) {
        return;
    }
    memset(&info, 0, sizeof(info));

    const profile_default_t *profile = profile_default_get(regular, profile_index);

    if (profile->name != NULL) {
        strcpy(info.name, profile->name);
    }
    if (profile->color != NULL) {
        info.has_color = true;
        info.color = *profile->color;
    }
    if (profile->icon != NULL) {
        info.has_f3 = true;
        strcpy(info.f3, profile->icon);
    }
    profile_default_write_pb_file(profile_index, "i", &ProfileInfo_msg, PROFILE_INFO_ENCODED_MAX, &info);
}

void profile_default_create_key_pairs(const profile_default_set_t *regular, uint32_t profile_index,
                                      bool *created)
{
    char path[28];
    AdvancedKeys advanced_keys;

    storage_format_profile_path(path, PROFILE_ID_RAW(profile_index, PROFILE_NS_BUILTIN), "ak2");
    memset(&advanced_keys, 0, sizeof(advanced_keys));

    const profile_default_t *profile = profile_default_get(regular, profile_index);

    if (profile->key_pair_count == 0u) {
        return;
    }
    for (uint32_t i = 0u; i < profile->key_pair_count; i++) {
        const profile_default_key_pair_t *pair = &profile->key_pairs[i];
        AdvancedKey *entry = &advanced_keys.entries[i];

        entry->which_kind = AdvancedKey_depth_priority_tag;
        entry->kind.depth_priority.second_key_id = pair->second_key_id;
        entry->kind.depth_priority.both_bottomed_resolution = pair->both_bottomed_resolution;
        entry->key_id = pair->key_id;
        entry->layer = 0u;
    }
    advanced_keys.entries_count = profile->key_pair_count;

    if (storage_file_exists(path) == 0) {
        *created = true;
        storage_save_advanced_keys(PROFILE_ID_RAW(profile_index, PROFILE_NS_BUILTIN), &advanced_keys);
    }
}

void profile_default_create_lightbar(const profile_default_set_t *regular, uint32_t profile_index,
                                     bool *created)
{
    if (!profile_default_file_missing(profile_index, "lb")) {
        return;
    }
    lightbar_write_default_profile_file(profile_index, &profile_default_get(regular, profile_index)->lightbar,
                                        created);
}

void profile_default_create_keymaps(uint8_t layout_variant, bool *created)
{
    uint8_t keymap[LAYOUT_ROWS][LAYOUT_KEYMAP_COLS];

    for (uint32_t layer = 0u; layer < BOARD_DEFAULT_KEYMAP_LAYERS; layer++) {
        if (!profile_default_numbered_file_missing(0u, "ml%d", (int)layer)
            || (layer >= PROFILE_KEYMAPS_ALWAYS_WRITTEN && !*created)) {
            continue;
        }
        memcpy(keymap, k_layout_default_keymap_layers[layer], sizeof(keymap));
        if (layer == 0u && layout_variant < BOARD_LAYOUT_VARIANT_COUNT) {
            const layout_variant_t *variant = &k_layout_variants[layout_variant];

            for (uint32_t i = 0u; i < variant->layer0_patch_count; i++) {
                const layout_keymap_patch_t *patch = &variant->layer0_patches[i];

                keymap[patch->row][patch->col] = patch->key_code;
            }
        }
        storage_write_keymap_layer(PROFILE_ID_RAW(0u, PROFILE_NS_BUILTIN), keymap, layer);
        *created = true;
    }
}

void profile_default_create_global(void)
{
    GlobalSettings global;

    if (storage_file_exists("global") != 0) {
        return;
    }
    memset(&global, 0, sizeof(global));
    global.has_idle = true;
    global.idle.dim_after_minutes = PROFILE_GLOBAL_IDLE_DIM_MINUTES_DEFAULT;
    global.idle.off_after_minutes = PROFILE_GLOBAL_IDLE_OFF_MINUTES_DEFAULT;
    global.idle.has_off_enabled = true;
    global.idle.off_enabled = true;
    global.has_num_lock_indicator_inverted = true;
    global.num_lock_indicator_inverted = false;
    profile_default_write_root_pb_file("global", &GlobalSettings_msg, PROFILE_GLOBAL_ENCODED_MAX, &global);
}

void profile_default_create_lightbar_global(const profile_default_set_t *regular)
{
    if (storage_file_exists("glb") != 0) {
        return;
    }
    lightbar_write_default_global_file(&profile_edition_default_set(regular)->lightbar_global);
}
