/*
 * signals_profile.c - the profile sections of feature `signals`: before a profile is loaded every
 * subscription is dropped (the subscribers of the new profile bind again); after it is loaded
 * its position and its colour are published.
 *
 * Depends on the profile module (section list, profile files).
 */
#include <string.h>

#include "profile/profile.h"
#include "rgb/signals/signals.h"
#include "storage/storage.h"

enum {
    PROFILE_SECTION_SIGNAL_SUBSCRIBERS    = PROFILE_SECTION_FN_ACTIONS - 1,
    PROFILE_SECTION_SIGNAL_ACTIVE_PROFILE = PROFILE_SECTION_GAMEPAD + 1,
    PROFILE_SECTION_SIGNAL_PROFILE_COLOR  = PROFILE_SECTION_GAMEPAD + 2,
};

static void signals_section_clear_subscribers(uint16_t profile_id, uint32_t default_index)
{
    signals_clear_subscribers();
}
PROFILE_SECTION(PROFILE_SECTION_SIGNAL_SUBSCRIBERS, signals_section_clear_subscribers);

/* Index and namespace of the profile -> RGB_SIGNAL_ACTIVE_PROFILE. */
static void signals_section_active_profile(uint16_t profile_id, uint32_t default_index)
{
    signals_publish_if_changed(RGB_SIGNAL_ACTIVE_PROFILE,
                               (float)(int)((PROFILE_ID_INDEX(profile_id) + 1) | (PROFILE_ID_NS(profile_id) << 8)));
}
PROFILE_SECTION(PROFILE_SECTION_SIGNAL_ACTIVE_PROFILE, signals_section_active_profile);

/* Colour of file "i" -> RGB_SIGNAL_PROFILE_COLOR; nothing without the file. */
static void signals_section_profile_color(uint16_t profile_id, uint32_t default_index)
{
    ProfileInfo info;

    memset(&info, 0, sizeof(info));
    if (profile_read_pb_file(profile_id, "i", &ProfileInfo_msg, &info) != STORAGE_OK) {
        return;
    }

    /* The signal carries the raw 0xFFRRGGBB bits in its float. For a red component of
     * 0x80 or more (except pure 0x800000, which is -infinity) that bit pattern is a NaN
     * and the "changed" comparison is always true; for a smaller red component it is an
     * ordinary negative number and an unchanged colour is not published again. */
    uint32_t argb = 0xFF000000u | ((uint32_t)info.color.r << 16) | ((uint32_t)info.color.g << 8) | info.color.b;
    float value;

    memcpy(&value, &argb, sizeof(value));
    signals_publish_if_changed(RGB_SIGNAL_PROFILE_COLOR, value);
}
PROFILE_SECTION(PROFILE_SECTION_SIGNAL_PROFILE_COLOR, signals_section_profile_color);
