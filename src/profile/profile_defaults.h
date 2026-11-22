/*
 * profile_defaults.h - what the factory defaults of a board are made of. The board's
 * defaults/profile_defaults.c describes its factory profiles as data (profile_default_set_t) and
 * defines the four functions of profile/profile.h with the helpers below:
 * profile_fill_default_rgb_settings, profile_copy_default_base_colors and profile_fill_default_cfg
 * are the fallbacks of the loaders, profile_create_factory_defaults writes the default files of
 * the first boot in the board's order. The helpers (profile_default_files.c) take the board's data
 * set of a regular unit; feature `edition` replaces it by the set of the edition on a unit of a
 * special edition (profile_edition_default_set). The content of a feature's files is data of the
 * feature's type (empty without the feature), written by the feature's writers.
 *
 * Depends on profile/profile_types.h (settings messages, PROFILE_BUILTIN_MAX) and on the public
 * header of feature `lightbar` (factory content of its files).
 */
#ifndef PROFILE_PROFILE_DEFAULTS_H
#define PROFILE_PROFILE_DEFAULTS_H

#include <stdbool.h>
#include <stdint.h>

#include "profile/profile_types.h"
#include "rgb/lightbar/lightbar.h"

/* Factory values of file "cfg"; written over a ProfileConfig that holds the message defaults,
 * every other field is left as it is. */
typedef struct {
    uint16_t actuation_point;
    uint16_t rapid_trigger_sensitivity;         /* non-zero: rapid trigger on, with this sensitivity */
    uint16_t rapid_trigger_release_sensitivity; /* non-zero: a separate release sensitivity */
    bool rapid_trigger_non_continuous;          /* written with its has-flag set */
    bool scan_mode_parallel;                    /* true: scan mode on, parallel */
} profile_default_cfg_t;

/* One entry of file "act": the actuation point of key id (row << 5 | col). */
typedef struct {
    uint8_t key_id;
    uint8_t actuation_point;
} profile_default_actuation_t;

/* One entry of file "ak2": a depth-priority pair on layer 0, owned by key_id. */
typedef struct {
    uint8_t key_id;
    uint8_t second_key_id;
    TwoKeyResolution both_bottomed_resolution;
} profile_default_key_pair_t;

/* Factory content of one built-in profile. A file whose list is empty is not written at the
 * first boot. */
typedef struct {
    /* file "rgb" */
    uint8_t brightness;
    void (*fill_effects)(RgbSettings *rgb_settings); /* sets the effect layers; NULL: no effect */
    /* file "clr": STORAGE_BASE_COLORS_SIZE bytes; NULL: none (a loader keeps its buffer) */
    const uint16_t *base_colors;
    /* file "cfg" */
    profile_default_cfg_t cfg;
    /* file "act" */
    const profile_default_actuation_t *actuation_points;
    uint8_t actuation_point_count;
    /* file "ak2" */
    const profile_default_key_pair_t *key_pairs;
    uint8_t key_pair_count;
    /* file "i": name (NULL: empty), colour (NULL: none), icon id (NULL: none; the firmware does
     * not read it) */
    const char *name;
    const RgbColor *color;
    const char *icon;
    /* file "lb" of feature lightbar */
    lightbar_profile_default_t lightbar;
} profile_default_t;

/* The factory profiles of one kind of unit. */
typedef struct profile_default_set {
    profile_default_t profiles[PROFILE_BUILTIN_MAX]; /* by built-in profile index */
    profile_default_t other;                         /* every larger index (cfg of an lp profile) */
    /* An index of PROFILE_BUILTIN_MAX or more with (index & profile0_alias_mask) == 0 gets
     * profiles[0]; 0: there is no such index. No caller passes one (the loaders pass at most
     * 255). Matches stock firmware 2.14.1, whose special-edition defaults test for the indices 0
     * and 2 on bits 0 and 2..7 only. */
    uint32_t profile0_alias_mask;
    /* root file "glb" of feature lightbar */
    lightbar_global_default_t lightbar_global;
} profile_default_set_t;

/* ---- Loader fallbacks: `regular` is the board's set of a regular unit ---------------------- */

/* Brightness and indicator colours of file "rgb", then the effect layers. The other fields of
 * rgb_settings are left as they are. */
void profile_default_fill_rgb(const profile_default_set_t *regular, uint32_t profile_index,
                              RgbSettings *rgb_settings);
/* Copies the base colour map of file "clr"; a profile without one leaves `colors` untouched. */
void profile_default_copy_base_colors(const profile_default_set_t *regular, uint32_t profile_index,
                                      uint16_t *colors);
/* Writes the factory values of file "cfg" into cfg. */
void profile_default_fill_cfg(const profile_default_set_t *regular, uint32_t profile_index, ProfileConfig *cfg);

/* ---- First boot: one file of built-in profile profile_index each, written only if missing ---- */
/* `created` is carried across the files: some are only written once another one was created. */

/* Encode `msg` and write it as file `name` of built-in profile profile_index, or as the root
 * file `name`; nothing if the storage is not mounted. The encode result is not checked (a failed
 * encode writes the partial output). Matches stock firmware 2.14.1. */
void profile_default_write_pb_file(uint32_t profile_index, const char *name, const pb_msgdesc_t *msgdesc,
                                   uint32_t max_size, const void *msg);
void profile_default_write_root_pb_file(const char *name, const pb_msgdesc_t *msgdesc, uint32_t max_size,
                                        const void *msg);

bool profile_default_file_missing(uint32_t profile_index, const char *name);
void profile_default_create_rgb(const profile_default_set_t *regular, uint32_t profile_index, bool *created);
void profile_default_create_base_colors(const profile_default_set_t *regular, uint32_t profile_index,
                                        bool *created);
/* File "r<layer>" with `colors` (STORAGE_LAYER_COLORS_SIZE bytes); only after a file was created. */
void profile_default_create_layer_colors(uint32_t profile_index, uint32_t layer, const uint16_t *colors,
                                         bool *created);
void profile_default_create_cfg(const profile_default_set_t *regular, uint32_t profile_index, bool *created);
/* File "act" (the test for it is made for every profile, also when its list is empty). */
void profile_default_create_actuation(const profile_default_set_t *regular, uint32_t profile_index,
                                      bool *created);
/* File "i"; only after a file was created. */
void profile_default_create_info(const profile_default_set_t *regular, uint32_t profile_index, bool created);
/* File "ak2" (tested for only when the profile has key pairs). */
void profile_default_create_key_pairs(const profile_default_set_t *regular, uint32_t profile_index,
                                      bool *created);
/* File "lb" of feature lightbar, written by the feature (lightbar_write_default_profile_file)
 * with the profile's `lightbar` content. Only a board with the feature calls it. */
void profile_default_create_lightbar(const profile_default_set_t *regular, uint32_t profile_index,
                                     bool *created);
/* Files "ml0".. of built-in profile 0 from the board's default key map layers
 * (k_layout_default_keymap_layers), layer 0 with the key codes of physical variant
 * layout_variant patched in. Layers 0 and 1 are written whenever missing, further layers only
 * after a file was created. */
void profile_default_create_keymaps(uint8_t layout_variant, bool *created);
/* Root file "global": idle dimming and switch-off, Num Lock indicator. */
void profile_default_create_global(void);
/* Root file "glb" of feature lightbar, written by the feature (lightbar_write_default_global_file)
 * with the set's `lightbar_global` content. Only a board with the feature calls it. */
void profile_default_create_lightbar_global(const profile_default_set_t *regular);

#endif /* PROFILE_PROFILE_DEFAULTS_H */
