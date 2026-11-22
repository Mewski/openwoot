/*
 * profile: types and constants of the profile / settings module.
 */
#ifndef PROFILE_TYPES_H
#define PROFILE_TYPES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "proto/settings.pb.h"

/* ------------------------------------------------------------------------- */
/* Profile id                                                                */
/* ------------------------------------------------------------------------- */

/* Namespace of a profile id = directory scheme on the littlefs volume
 * (storage_format_profile_path, storage_count_profile_dirs). */
enum profile_ns {
    PROFILE_NS_BUILTIN = 0, /* directory "<index>/",    index 0..3 */
    PROFILE_NS_LP      = 1, /* directory "lp/<index>/", index 0..9; any other value: invalid (empty path) */
};

#define PROFILE_NS_COUNT        2u  /* profile_activate rejects ns > 1 */
#define PROFILE_BUILTIN_MAX     4u  /* directories "0".."3" */
#define PROFILE_LP_MAX          10u /* directories "lp/0".."lp/9" */

/* Number of built-in profiles created on first boot; a unit of a special edition can have
 * another number (profile_edition_factory_profile_count). */
#define PROFILE_FACTORY_COUNT       2u

/*
 * Profile id. Every per-profile function takes it as one 16-bit value
 * (`uint16_t profile_id`): low byte = index inside the namespace, high byte =
 * namespace (enum profile_ns).
 *
 * The type is a 2-byte object with alignment 1, so globals of this type may
 * sit at odd addresses; the two halves are usually accessed as single bytes.
 * `raw` is the value that goes into the uint16_t parameters.
 */
typedef union __attribute__((packed)) {
    uint16_t raw;
    struct {
        uint8_t index; /* 0x0: profile index inside the namespace */
        uint8_t ns;    /* 0x1: enum profile_ns */
    };
} profile_id_t;

_Static_assert(sizeof(profile_id_t) == 2, "profile_id_t size");
_Static_assert(_Alignof(profile_id_t) == 1, "profile_id_t alignment");
_Static_assert(offsetof(profile_id_t, raw) == 0, "profile_id_t.raw");
_Static_assert(offsetof(profile_id_t, index) == 0, "profile_id_t.index");
_Static_assert(offsetof(profile_id_t, ns) == 1, "profile_id_t.ns");

#define PROFILE_ID_RAW(index, ns)   ((uint16_t)((uint8_t)(index) | ((uint16_t)(uint8_t)(ns) << 8)))
#define PROFILE_ID_INDEX(raw)       ((uint8_t)(raw))
#define PROFILE_ID_NS(raw)          ((uint8_t)((uint16_t)(raw) >> 8))

/* "No profile" / "the active profile" in the protocol handlers. */
#define PROFILE_ID_NONE             0xFFFFu

/* g_profile_counts index = enum profile_ns. */
#define PROFILE_COUNTS_BUILTIN      PROFILE_NS_BUILTIN
#define PROFILE_COUNTS_LP           PROFILE_NS_LP

/* ------------------------------------------------------------------------- */
/* Lock mode (factory device info)                                           */
/* ------------------------------------------------------------------------- */

/* g_profile_lock_mode (device info tag 4): values 1 and 3 lock the switch
 * profiles and host signals. */
#define PROFILE_LOCK_MODE_IS_LOCKED(mode)   ((((uint8_t)(mode)) & 0xFDu) == 1u)

/* ------------------------------------------------------------------------- */
/* nanopb messages used by this module (types from settings.pb.h)            */
/* ------------------------------------------------------------------------- */

/*
 * File "global" = g_profile_global_settings: message GlobalSettings, 51 bytes.
 * Offsets as used by the code (generated field in brackets):
 *   0x00 bool      [has_ of tag 1]  idle handling configured; rgb_update_frame runs the idle timer only if set
 *   0x01 uint8_t   [tag 1 . tag 1]  idle minutes until the LEDs dim, default 5
 *   0x02 uint8_t   [tag 1 . tag 2]  idle minutes until the LEDs switch off, default 10
 *   0x03 bool      [tag 1 . has_ of tag 3]
 *   0x04 bool      [tag 1 . tag 3]  switch-off enabled, default 1; only tested when its has-flag is set
 *   0x05 bool      [has_ of tag 2]
 *   0x06 bool      [tag 2]          Num Lock indicator polarity: indicator shows "Num Lock off" when the field is
 *                                   absent or non-zero, "Num Lock on" when present and 0
 *   0x07 bool      [has_ of tag 3]
 *   0x08 bool      [tag 3]          has && value is latched in g_profile_global_option3_active
 *                                   (rebound_filter_option_init, protocol_cmd_set_global_settings)
 *   0x09 bool      [has_ of tag 4]
 *   0x0A char[41]  [tag 4]          string, only stored and returned (no reader in the firmware)
 * Defaults (nanopb defaults = initial value of the global): 0x01 = 5, 0x02 = 10, 0x04 = 1, rest 0.
 */
_Static_assert(sizeof(GlobalSettings) == 51, "size of message GlobalSettings");

#define PROFILE_GLOBAL_IDLE_DIM_MINUTES_DEFAULT 5u
#define PROFILE_GLOBAL_IDLE_OFF_MINUTES_DEFAULT 10u

/*
 * File "<dir>/cfg": file message ProfileConfigFile with one member, the message ProfileConfig, 18 bytes.
 * It has no RAM copy: profile_apply_cfg scatters it into the globals below,
 * profile_export_cfg collects it again. Offsets (generated field in brackets):
 *   0x00 uint16_t  [tag 1]          actuation point            <-> g_keys_actuation_point; default 180
 *   0x02 bool      [tag 2]          keyboard in gamepad mode   <-> g_hid_kbd_enabled_in_gamepad_mode; default 1
 *   0x03 bool      [tag 3]          rapid trigger enabled      <-> g_keys_rapid_trigger_enabled
 *   0x04 bool      [has_ of tag 4]  export writes 1; gates the clamp of tag 4 in profile_sanitize_cfg_distances
 *   0x06 uint16_t  [tag 4]          rapid trigger sensitivity  <-> g_keys_rapid_trigger_params +0; default 65
 *   0x08 bool      [tag 5]          scan mode enabled; level = tag 5 ? min(tag 9, 1) : 0 -> g_run_state_flags bits 7:5
 *   0x09 bool      [has_ of tag 6]  export writes 1
 *   0x0A uint8_t   [tag 6]          tap duration preset        <-> g_keys_tap_duration_preset; default 2
 *   0x0B bool      [has_ of tag 7]                             <-> g_keys_rapid_trigger_params +6 (has_continuous)
 *   0x0C bool      [tag 7]          inverted "continuous"      <-> g_keys_rapid_trigger_params +7 = value ^ 1
 *   0x0D bool      [has_ of tag 8]                             <-> g_keys_rapid_trigger_params +2 (has_release_sensitivity)
 *   0x0E uint16_t  [tag 8]          release sensitivity        <-> g_keys_rapid_trigger_params +4
 *   0x10 bool      [has_ of tag 9]  export writes 1
 *   0x11 uint8_t   [tag 9]          scan mode level; default 1; export writes g_run_state_flags >> 5
 */
_Static_assert(sizeof(ProfileConfig) == 18, "size of message ProfileConfig");
_Static_assert(sizeof(ProfileConfigFile) == 18, "size of message ProfileConfigFile");

/* Clamp constants of profile_sanitize_cfg_distances. Distances are
 * 12-bit fractions of the switch travel; travel is in 0.05 mm units
 * (k_analog_switch_travel), so NUMERATOR / travel = fraction for a fixed length. */
#define PROFILE_CFG_ACTUATION_DEFAULT_Q12       0x605u  /* 1541 / 4095 */
#define PROFILE_CFG_MIN_DISTANCE_NUM            0x1FFEu /* 0.10 mm: minimum of all three distances */
#define PROFILE_CFG_RT_DEFAULT_NUM              0x3FFCu /* 0.20 mm: default of tag 4 when tag 8 is present */
#define PROFILE_CFG_RT_DEFAULT_NO_RELEASE_NUM   0x7FF8u /* 0.40 mm: default of tag 4 when tag 8 is absent */
#define PROFILE_CFG_RT_RELEASE_DEFAULT_NUM      0xAFF5u /* 0.55 mm: default of tag 8 */

/* File "<dir>/i" (profile name, colour, a string): message ProfileInfo, 57 bytes.
 * Read on the stack of profile_activate, never kept in RAM. */
_Static_assert(sizeof(ProfileInfo) == 57, "size of message ProfileInfo");

#endif /* PROFILE_TYPES_H */
