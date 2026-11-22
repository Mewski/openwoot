/*
 * profile: activation. profile_activate makes a profile the active one and runs
 * the profile sections (profile.h), which load each part of it into the RAM state
 * of the module that owns it.
 */
#include "profile/profile.h"

/*
 * Makes profile_id the active profile and loads all of its files. Ignored if
 * the namespace is unknown or the profile does not exist. Called at start-up by
 * rgb_start_and_activate_profile and by the main loop for a pending profile switch.
 */
void profile_activate(uint16_t profile_id)
{
    uint8_t ns = PROFILE_ID_NS(profile_id);
    uint8_t index = PROFILE_ID_INDEX(profile_id);

    if (ns >= PROFILE_NS_COUNT) {
        return;
    }
    if (g_profile_counts[ns] <= index) {
        return;
    }

    /* Remember where the profile keys return to. */
    if (ns != PROFILE_NS_BUILTIN) {
        if (g_profile_active_id.ns == PROFILE_NS_BUILTIN) {
            g_profile_lp_return_id = g_profile_active_id;
        }
    } else if (index != 0u) {
        g_profile_toggle_target_id.index = index;
        g_profile_toggle_target_id.ns = PROFILE_NS_BUILTIN;
    }
    g_profile_active_id.index = index;
    g_profile_active_id.ns = ns;

    /* Factory defaults exist for the built-in profiles only; "lp" profiles use those of profile 0. */
    uint32_t default_index = ns != PROFILE_NS_BUILTIN ? 0u : index;
    REGISTRY_FOREACH_ORDERED(profile_sections, profile_section_t, section) {
        section->load(profile_id, default_index);
    }
}
