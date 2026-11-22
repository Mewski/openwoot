/*
 * edition.h - public header of feature `edition`: special-edition units. A unit whose factory
 * serial number carries one of the board's edition codes is a unit of the board's special
 * edition; the edition changes the USB product string, the number of factory profiles, their
 * content and the default switch type. A board lists the feature in board.mk, gives its
 * constants in the edition block of board.h and the content of the edition's factory profiles
 * as k_profile_edition_default_set in its factory defaults (defaults/profile_defaults.c);
 * without the feature every unit is a regular one and the functions below answer with the
 * values of a regular unit.
 *
 * Depends on profile/profile_types.h (number of factory profiles of a regular unit),
 * profile/profile_defaults.h (factory profile data set) and, with the feature, on the storage
 * module (device info) and the edition constants of board.h.
 */
#ifndef PROFILE_EDITION_EDITION_H
#define PROFILE_EDITION_EDITION_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "profile/profile_defaults.h"
#include "profile/profile_types.h"

/* Default switch type of a regular unit: the type of keys that have no switch profile entry. */
#define PROFILE_EDITION_REGULAR_SWITCH_TYPE 0u

#if FEATURE_EDITION

/* Factory profiles of a unit of the special edition: data of the board's factory defaults. */
extern const profile_default_set_t k_profile_edition_default_set;

/* True on a unit of the special edition. Reads the device info on every call until one read
 * succeeds; an unreadable record counts as a regular unit. */
bool profile_edition_is_special(void);

/* USB product string of the unit: that of the edition on a special unit, NULL on a regular one
 * (the caller keeps BOARD_USB_PRODUCT). */
const char *profile_edition_product_string(void);

/* Number of built-in profiles the first boot creates: that of the edition on a special unit,
 * PROFILE_FACTORY_COUNT on a regular one. */
uint8_t profile_edition_factory_profile_count(void);

/* Factory profiles of the unit: k_profile_edition_default_set on a special unit, `regular` (the
 * board's set of a regular unit) on a regular one. */
const profile_default_set_t *profile_edition_default_set(const profile_default_set_t *regular);

/* Switch type of keys without a switch profile entry: that of the edition on a special unit,
 * PROFILE_EDITION_REGULAR_SWITCH_TYPE on a regular one. Decided at the first call and kept,
 * also when the device info could not be read then. */
uint8_t profile_edition_default_switch_type(void);

#else

static inline bool profile_edition_is_special(void)
{
    return false;
}

static inline const char *profile_edition_product_string(void)
{
    return NULL;
}

static inline uint8_t profile_edition_factory_profile_count(void)
{
    return PROFILE_FACTORY_COUNT;
}

static inline const profile_default_set_t *profile_edition_default_set(const profile_default_set_t *regular)
{
    return regular;
}

static inline uint8_t profile_edition_default_switch_type(void)
{
    return PROFILE_EDITION_REGULAR_SWITCH_TYPE;
}

#endif /* FEATURE_EDITION */

#endif /* PROFILE_EDITION_EDITION_H */
