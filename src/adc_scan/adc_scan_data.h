/*
 * adc_scan_data.h - RAM globals of the adc_scan module: scan state machine of the conversion
 * interrupt, result buffer, scan column table, scan event queue.
 *
 * One extern per global; the definitions and the initial values are in adc_scan_data.c. Globals
 * shared between the ADC interrupt and the main loop are volatile.
 */
#ifndef ADC_SCAN_DATA_H
#define ADC_SCAN_DATA_H

#include "adc_scan/adc_scan_types.h"

/* Start-up sweep: conversions left for the current position; reloaded with 100 when it reaches 0
 * (adc_scan_sweep_step). */
extern uint8_t g_adc_scan_sweep_samples_left;

/* Scan mode for which the key records currently hold the active calibration (rest_level /
 * noise_band); 0xFF = none. Written only by adc_scan_set_mode. */
extern uint8_t g_adc_scan_mode_applied;

/* Key event ring written by the ADC scan paths (adc_scan_event_push) and read by the main loop
 * (adc_scan_event_pop). */
extern adc_scan_event_t g_adc_scan_event_queue[ADC_SCAN_EVENT_QUEUE_LEN];

/* Index of the newest ring entry, -1 = empty. Written by the ADC interrupt. Volatile: reloaded for
 * every use. */
extern volatile int8_t g_adc_scan_event_head;

/* Index of the next ring entry to read, -1 = empty. Volatile: reloaded for every use. */
extern volatile int8_t g_adc_scan_event_tail;

/* Ring index of the end-of-cycle marker reserved by the ADC interrupt when a scan cycle is complete
 * (adc_scan_event_reserve); -1 while the cycle is running. */
extern volatile int8_t g_adc_scan_event_scan_end;

/* 1 from adc_scan_start_cycle until adc_scan_event_pop reaches the end-of-cycle marker; the main
 * loop starts a new cycle only when 0. Volatile: adc_scan_start_cycle stores 0 and then 1. */
extern volatile uint8_t g_adc_scan_cycle_busy;

/* Scan mode wanted by the settings / USB state code (profile_apply_cfg, tud_resume_cb,
 * usb_set_suspended): g_run_state_flags >> 5, or 0 while suspended. Applied by
 * adc_scan_start_cycle. */
extern uint8_t g_adc_scan_mode_requested;

/* 1 = adc_scan_start_cycle must call adc_scan_set_mode(g_adc_scan_mode_requested) before the next
 * cycle; also set after the start-up sweep. */
extern uint8_t g_adc_scan_mode_change_pending;

/* Result buffer of the converter (hal_scan_init), written by hardware; one word per result, read
 * through hal_scan_sample_value and hal_scan_sample_reference. [0] in single-input mode, [n] =
 * input g_adc_scan_parallel_mux_order[n] in parallel mode. */
extern uint32_t g_adc_scan_dma_samples[ADC_SCAN_MUX_COUNT];

/* 1 from a scan mode switch until g_adc_scan_settle_cycles further scan cycles have passed; key
 * evaluation is skipped meanwhile. Volatile: written by the main loop, read by the ADC interrupt. */
extern volatile uint8_t g_adc_scan_settling;

/* [mux address][mux index] -> key id, key record, scan position; built by
 * adc_scan_column_table_init from g_layout_scanpos_to_keyid. */
extern adc_scan_column_entry_t g_adc_scan_column_table[ADC_SCAN_MUX_CHANNELS][ADC_SCAN_MUX_COUNT];

/* 0 = one multiplexer / ADC input per conversion, 1 = all inputs per trigger
 * (adc_scan_set_mode); selects the interrupt path adc_scan_process_single_sample /
 * adc_scan_process_parallel_samples. */
extern uint8_t g_adc_scan_parallel_mode;

/* 1 while the ADC interrupt runs the start-up baseline sweep; set by adc_scan_start_sweep,
 * cleared by adc_scan_sweep_step. Volatile: adc_scan_start_cycle busy-waits on it. */
extern volatile uint8_t g_adc_scan_calibrating;

/* Has no effect: cleared together with g_adc_scan_calibrating at the end of the start-up sweep;
 * adc_scan_start_sweep reads it and discards the value (a volatile read); never set
 * non-zero. */
extern volatile uint8_t g_adc_scan_sweep_aux_flag;

/* Currently enabled analog multiplexer / ADC input 0 .. ADC_SCAN_MUX_COUNT - 1 (single-input mode
 * and sweep). */
extern uint32_t g_adc_scan_mux_index;

/* Current multiplexer address 0 .. ADC_SCAN_MUX_CHANNELS - 1 on the address lines. */
extern uint32_t g_adc_scan_mux_channel;

/* Start-up sweep phase: 0 = one input per conversion (every multiplexer x every address in use),
 * 1 = all inputs per trigger (every address in use). Set when the phase-1 multiplexer index wraps
 * to 0; never cleared. */
extern uint8_t g_adc_scan_sweep_parallel_phase;

/* 1 once the parallel converter sequence and g_adc_scan_parallel_mux_order are configured
 * (sweep or adc_scan_set_mode). */
extern uint8_t g_adc_scan_parallel_seq_ready;

/* Multiplexer index sampled at each converter sequence rank in parallel mode (a copy of the
 * board's k_adc_scan_parallel_mux_order). */
extern uint8_t g_adc_scan_parallel_mux_order[ADC_SCAN_MUX_COUNT];

/* Set by adc_scan_sweep_step when the start-up sweep has finished; consumed by
 * adc_scan_start_cycle. */
extern uint8_t g_adc_scan_sweep_done;

/* Single-input scan: column table entry of the key whose conversion is in flight (evaluated by the
 * next adc_scan_process_single_sample call); NULL before the first conversion. */
extern adc_scan_column_entry_t *g_adc_scan_pending_entry;

/* Parallel scan: hysteresis-filtered magnitude of each input at the current address. */
extern uint16_t g_adc_scan_parallel_filtered[ADC_SCAN_MUX_COUNT];

/* Parallel scan: unfiltered magnitude (adc_scan_magnitude) of each input at the current
 * address. */
extern uint16_t g_adc_scan_parallel_raw[ADC_SCAN_MUX_COUNT];

/* Scan mode last given to adc_scan_set_mode: 0 = single input, 1..7 = parallel
 * (g_run_state_flags >> 5). Read by analog_apply_noise_recalibration. */
extern uint8_t g_adc_scan_mode;

/* Countdown (starts at 3) of scan cycles left in the settling phase. */
extern uint8_t g_adc_scan_settle_cycles;

#endif /* ADC_SCAN_DATA_H */
