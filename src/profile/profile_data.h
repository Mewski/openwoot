/*
 * profile: RAM globals (extern declarations). Definitions and descriptions:
 * profile_data.c.
 */
#ifndef PROFILE_DATA_H
#define PROFILE_DATA_H

#include "profile_types.h"

/* Initialised */
extern profile_id_t g_profile_toggle_target_id;
extern profile_id_t g_profile_lp_return_id;
extern uint8_t      g_profile_lp_selected;
extern profile_id_t g_profile_lp_selected_id;
extern uint8_t      g_profile_switch_pending;
extern profile_id_t g_profile_switch_target;
extern profile_id_t g_profile_active_id;
extern uint8_t      g_profile_counts[PROFILE_NS_COUNT];
extern GlobalSettings g_profile_global_settings;

/* Zero-initialised */
extern uint8_t      g_profile_global_option3_active;
extern uint8_t      g_profile_lock_mode;

#endif /* PROFILE_DATA_H */
