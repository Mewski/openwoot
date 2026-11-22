/*
 * core: the two run-state bytes.
 */
#include "core/run_state.h"

/* Run flags. Written from main-loop context only (the restart countdown of main,
 * settings_load_and_apply, profile_apply_cfg, and the USB call-backs run by tud_task). */
uint8_t g_run_state_flags = RUN_STATE_FLAGS_INIT;

/* Option flags. Bit 0 is read by the scan paths in the conversion interrupt. */
uint8_t g_run_state_options = 0;
