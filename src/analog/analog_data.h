/*
 * analog_data.h - RAM globals of the analog module: per-key records, switch profiles, idle noise
 * recalibration state.
 *
 * One extern per global; the definitions and the initial values are in analog_data.c. Globals
 * shared between the ADC interrupt and the main loop are volatile.
 */
#ifndef ANALOG_DATA_H
#define ANALOG_DATA_H

#include "analog/analog_types.h"

/* enum analog_recal_pattern: how the idle noise recalibration groups keys into batches. Chosen by
 * adc_scan_set_mode, set to 1 after a complete pass (adc_scan_start_cycle). */
extern uint8_t g_analog_recal_pattern;

/* RAM copy of root file "sw" (analog_load_switch_profiles; defaults written by
 * analog_switch_profiles_init if the file is missing): up to 30 key groups with own switch
 * description, keyboard-wide default description at +0x298, version at +0x29E. */
extern SwitchProfiles g_analog_switch_profiles;

/* Per-key records, index = scan position (mux address + 16 * mux index). Initialised by
 * analog_key_records_init, updated by the ADC interrupt and the key task. */
extern analog_key_state_t g_analog_key_states[ANALOG_KEY_COUNT];

/* Index of the key batch currently being recalibrated. Volatile: re-read after every store. */
extern volatile uint8_t g_analog_recal_batch;

/* Keyboard-wide run-time switch params, built by analog_apply_switch_profiles from the default
 * description of file "sw"; used for keys with has_switch_params == 0. */
extern analog_switch_params_t g_analog_default_switch_params;

/* Set by the scan paths when a key moved or a noise statistic became unstable: recalibration is
 * blocked for 5000 ms after g_analog_recal_hold_stamp_ms. */
extern uint8_t g_analog_recal_hold;

/* Uptime of the last key activity (ADC interrupt) or of the last batch start
 * (adc_scan_start_cycle). */
extern uint32_t g_analog_recal_hold_stamp_ms;

/* 1 while a recalibration batch is sampling (keys with recal_active feed their noise statistics
 * from the ADC interrupt). Volatile: cleared by the ADC interrupt, read by the main loop. */
extern volatile uint8_t g_analog_recal_running;

/* 1 after a complete recalibration pass; the next pass starts 20000 ms after
 * g_analog_recal_pass_done_stamp_ms. */
extern uint8_t g_analog_recal_pass_done;

/* A recalibration pass has finished while the scan mode level (g_run_state_flags >> 5) was 0. */
extern uint8_t g_analog_recal_done_single;

/* A recalibration pass has finished while the scan mode level was not 0. */
extern uint8_t g_analog_recal_done_parallel;

/* Uptime at which the last recalibration pass finished. */
extern uint32_t g_analog_recal_pass_done_stamp_ms;

/* Selects what is stored in g_analog_recal_pass_end_time: 0 uptime ms, 1 microsecond count; only
 * ever written 0. */
extern uint8_t g_analog_recal_pass_time_source;

/* Uptime at the start of the current recalibration pass (written, never read). */
extern uint32_t g_analog_recal_pass_start_ms;

/* 1 while a pass is being timed (start in g_analog_recal_pass_start_ms). */
extern uint8_t g_analog_recal_pass_timing;

/* Time stamp taken when a recalibration pass ends (written, never read). */
extern uint32_t g_analog_recal_pass_end_time;

#endif /* ANALOG_DATA_H */
