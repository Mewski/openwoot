/*
 * profile: public functions of the profile / settings module, and the list of profile
 * sections that profile_activate runs when a profile becomes the active one.
 *
 * Depends on: core/registry.h, the nanopb settings messages, profile types and data.
 */
#ifndef PROFILE_PROFILE_H_
#define PROFILE_PROFILE_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "core/registry.h"
#include "proto/settings.pb.h"
#include "profile/profile_types.h"
#include "profile/profile_data.h"

/*
 * Profile sections. Each module that keeps state per profile registers a section next
 * to its loader with PROFILE_SECTION; profile_activate runs them in ascending order.
 * The order is that of the stock firmware 2.14.1 and decides the order of the file
 * reads, so the core slots below are the readable activation sequence; a feature
 * places its sections relative to a core slot.
 */
typedef void (*profile_section_fn)(uint16_t profile_id, uint32_t default_index);

/* One section: its slot and the function that loads it (default_index: factory profile to fall back on). */
typedef struct {
    uint16_t order;
    profile_section_fn load;
} profile_section_t;

REGISTRY_DECLARE(profile_sections, profile_section_t);

/* Registers `function` (profile_section_fn) as the profile section in slot `order`. */
#define PROFILE_SECTION(order, function) \
    REGISTRY_ENTRY(profile_sections, profile_section_t, function) = { (order), (function) }

/* Slots of the core sections, in the order in which they run. */
enum profile_section_order {
    PROFILE_SECTION_FN_ACTIONS             = 10,  /* states of the Fn key actions */
    PROFILE_SECTION_CONFIG                 = 20,  /* file "cfg" */
    PROFILE_SECTION_LIGHTING_SETTINGS      = 30,  /* file "rgb" */
    PROFILE_SECTION_BASE_COLORS            = 40,  /* file "clr" */
    PROFILE_SECTION_EFFECT_START           = 50,  /* effect layers from the lighting settings */
    PROFILE_SECTION_LAYER_COLORS           = 60,  /* files "r1".."r3" */
    PROFILE_SECTION_FRAME_RESET            = 70,  /* colour layer 0, full render */
    PROFILE_SECTION_ACTUATION_POINTS       = 80,  /* file "act" */
    PROFILE_SECTION_RAPID_TRIGGER          = 90,  /* files "prt", "prt2" */
    PROFILE_SECTION_KEY_MAPS               = 100, /* key map layers */
    PROFILE_SECTION_ADVANCED_KEYS          = 110, /* file "ak2" */
    PROFILE_SECTION_TIME_STAMP             = 120, /* time stamp of the key pass */
    PROFILE_SECTION_NEUTRAL_GAMEPAD_REPORT = 130, /* release every gamepad input */
    PROFILE_SECTION_GAMEPAD                = 140, /* files "gp", "gm" */
    PROFILE_SECTION_ACTIVATED_EVENT        = 150, /* "profile activated" event to the host */
};

/* Collects the live settings into a ProfileConfig (inverse of profile_apply_cfg). */
void profile_export_cfg(ProfileConfig *cfg);
/* Copies a ProfileConfig into the live settings; refresh_rgb != 0 repaints on a scan mode change. */
void profile_apply_cfg(const ProfileConfig *cfg, uint32_t refresh_rgb);
/* Clamps the distance fields of a ProfileConfig against the installed switch types. */
void profile_sanitize_cfg_distances(ProfileConfig *cfg);
/* Makes profile_id the active profile and loads all of its files. */
void profile_activate(uint16_t profile_id);
/* Reads nanopb file `name` of a profile into msg (for the loaders of the profile sections).
 * Returns STORAGE_ERR_NOT_MOUNTED without a file system, else the result of storage_read_pb_file. */
int profile_read_pb_file(uint16_t profile_id, const char *name, const pb_msgdesc_t *msgdesc, void *msg);
/* Fills the factory RGB settings of a built-in profile. */
void profile_fill_default_rgb_settings(uint32_t profile_index, RgbSettings *rgb_settings);
/* Copies the factory base colours of a built-in profile. */
void profile_copy_default_base_colors(uint32_t profile_index, uint16_t *colors);
/* Fills the factory ProfileConfig of a built-in profile. */
void profile_fill_default_cfg(uint32_t profile_index, ProfileConfig *cfg);
/* Creates the profile directories and files of a keyboard without stored settings. */
void profile_create_factory_defaults(void);
/* Loads the cfg file of a profile (with fallbacks) and applies it. */
void profile_load_and_apply_cfg(uint16_t profile_id);

#endif /* PROFILE_PROFILE_H_ */
