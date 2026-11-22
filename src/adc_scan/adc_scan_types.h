/*
 * adc_scan_types.h - types and constants of the `adc_scan` module (hall-sensor multiplexing,
 * ADC sampling, scan state machine in the ADC interrupt, scan event queue).
 *
 * The layouts are fixed by the _Static_assert lines that follow each type.
 */
#ifndef ADC_SCAN_TYPES_H
#define ADC_SCAN_TYPES_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "board.h"
#include "analog/analog_types.h"

/* ---- Scan geometry --------------------------------------------------------------------------- */

#define ADC_SCAN_MUX_COUNT        ((unsigned)BOARD_MUX_COUNT)          /* 16:1 analog multiplexers = ADC inputs sampled per address */
#define ADC_SCAN_MUX_CHANNELS     ((unsigned)BOARD_MUX_ADDRESSES_USED) /* multiplexer addresses in use, counted from 0 */
#define ADC_SCAN_POS_PER_MUX      ((unsigned)BOARD_MUX_POSITIONS)      /* scan position = mux address + ADC_SCAN_POS_PER_MUX * mux index */
#define ADC_SCAN_POSITIONS        ((unsigned)BOARD_SCAN_POSITIONS)     /* = ANALOG_KEY_COUNT; positions with an unused address never carry a key */
#define ADC_SCAN_NO_KEY           0xFFu /* g_layout_scanpos_to_keyid entry of a position without sensor */

#define ADC_SCAN_SWEEP_SAMPLES    100u  /* conversions per position in the start-up sweep (g_adc_scan_sweep_samples_left reload) */
#define ADC_SCAN_SETTLE_CYCLES    3u    /* scan cycles skipped after a mode switch (g_adc_scan_settle_cycles) */

/* Trigger delays in microseconds (hal_scan_trigger_after). */
#define ADC_SCAN_DELAY_FIRST_US        254u  /* first sweep conversion (adc_scan_start_sweep) */
#define ADC_SCAN_DELAY_ADDRESS_US      149u  /* sweep: after a mux address change */
#define ADC_SCAN_DELAY_MUX_US          54u   /* after switching to another multiplexer */
#define ADC_SCAN_DELAY_SAME_MUX_US     2u    /* next address on the same multiplexer / empty position / second conversion (feature recheck) */
#define ADC_SCAN_DELAY_SWEEP_REPEAT_US 1u    /* sweep: next conversion of the same position */
#define ADC_SCAN_CYCLE_TIMER_US        900u  /* stopwatch period at the end of a scan cycle (hal_scan_end_cycle) */

/* Scan mode given to adc_scan_set_mode (g_adc_scan_mode, g_adc_scan_mode_requested,
 * g_adc_scan_mode_applied). Values 1..7 are g_run_state_flags >> 5. */
enum adc_scan_mode {
    ADC_SCAN_MODE_SINGLE   = 0,     /* one multiplexer / ADC input per conversion */
    ADC_SCAN_MODE_PARALLEL = 1,     /* 1..7: all inputs per trigger; > 1 adds margins to rest level (+10) and noise band (+2) */
    ADC_SCAN_MODE_NONE     = 0xFF,  /* g_adc_scan_mode_applied before the first adc_scan_set_mode (.data) */
};

/* ---- Scan column table ------------------------------------------------------------------------ */

/* 12 bytes. Element of g_adc_scan_column_table[mux address][mux index], built at start-up from
 * g_layout_scanpos_to_keyid. The single-input path walks the table one entry at a time, the
 * parallel path takes the consecutive entries of one address. */
typedef struct adc_scan_column_entry {
    uint8_t key_id;                 /* +0x00 row << 5 | col; not written when the position has no key */
    analog_key_state_t *key_state;  /* +0x04 &g_analog_key_states[scan_pos], NULL if the position has no key */
    uint8_t scan_pos;               /* +0x08 mux address + ADC_SCAN_POS_PER_MUX * mux index; written only when a key exists */
} adc_scan_column_entry_t;

_Static_assert(sizeof(adc_scan_column_entry_t) == 12, "adc_scan_column_entry_t size");
_Static_assert(offsetof(adc_scan_column_entry_t, key_id) == 0, "adc_scan_column_entry_t.key_id");
_Static_assert(offsetof(adc_scan_column_entry_t, key_state) == 4, "adc_scan_column_entry_t.key_state");
_Static_assert(offsetof(adc_scan_column_entry_t, scan_pos) == 8, "adc_scan_column_entry_t.scan_pos");

/* ---- Scan event queue -------------------------------------------------------------------------- */

#define ADC_SCAN_EVENT_QUEUE_LEN   30    /* ring entries (index wrap at 29) */
#define ADC_SCAN_EVENT_INDEX_NONE  (-1)  /* head / tail / scan_end: ring empty / cycle not finished (stored as 0xFF) */

/* 12 bytes. Element of g_adc_scan_event_queue; written by adc_scan_event_push (ADC interrupt),
 * copied out by adc_scan_event_pop (main loop, interrupts masked). */
typedef struct adc_scan_event {
    uint8_t key_id;                 /* +0x00 row << 5 | col */
    analog_key_state_t *key_state;  /* +0x04 key record */
    uint16_t travel;                /* +0x08 0..4095 (analog_compute_key_travel), 0 for a suppressed rebound */
} adc_scan_event_t;

_Static_assert(sizeof(adc_scan_event_t) == 12, "adc_scan_event_t size");
_Static_assert(offsetof(adc_scan_event_t, key_id) == 0, "adc_scan_event_t.key_id");
_Static_assert(offsetof(adc_scan_event_t, key_state) == 4, "adc_scan_event_t.key_state");
_Static_assert(offsetof(adc_scan_event_t, travel) == 8, "adc_scan_event_t.travel");

/* Return value of adc_scan_event_pop. */
enum adc_scan_event_pop_result {
    ADC_SCAN_POP_EMPTY     = 0,  /* ring empty, scan cycle still running */
    ADC_SCAN_POP_CYCLE_END = 1,  /* read index reached g_adc_scan_event_scan_end; g_adc_scan_cycle_busy cleared */
    ADC_SCAN_POP_EVENT     = 2,  /* one event copied to the caller */
};

#endif /* ADC_SCAN_TYPES_H */
