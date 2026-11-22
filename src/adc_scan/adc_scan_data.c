/*
 * adc_scan_data.c - definitions of the RAM globals of the adc_scan module: scan state machine of
 * the ADC interrupt, DMA sample buffer, scan column table, scan event queue.
 *
 * Objects in .data carry their initial value; objects in .bss have no initialiser.
 */
#include "adc_scan/adc_scan_data.h"

/* ---- .data ------------------------------------------------------------------------------------ */

uint8_t g_adc_scan_sweep_samples_left = ADC_SCAN_SWEEP_SAMPLES;
uint8_t g_adc_scan_mode_applied = ADC_SCAN_MODE_NONE;

/* ---- .bss ------------------------------------------------------------------------------------- */

adc_scan_event_t g_adc_scan_event_queue[ADC_SCAN_EVENT_QUEUE_LEN];
volatile int8_t g_adc_scan_event_head;
volatile int8_t g_adc_scan_event_tail;
volatile int8_t g_adc_scan_event_scan_end;
volatile uint8_t g_adc_scan_cycle_busy;
uint8_t g_adc_scan_mode_requested;
uint8_t g_adc_scan_mode_change_pending;
uint32_t g_adc_scan_dma_samples[ADC_SCAN_MUX_COUNT];
volatile uint8_t g_adc_scan_settling;
adc_scan_column_entry_t g_adc_scan_column_table[ADC_SCAN_MUX_CHANNELS][ADC_SCAN_MUX_COUNT];
uint8_t g_adc_scan_parallel_mode;
volatile uint8_t g_adc_scan_calibrating;
volatile uint8_t g_adc_scan_sweep_aux_flag;
uint32_t g_adc_scan_mux_index;
uint32_t g_adc_scan_mux_channel;
uint8_t g_adc_scan_sweep_parallel_phase;
uint8_t g_adc_scan_parallel_seq_ready;
uint8_t g_adc_scan_parallel_mux_order[ADC_SCAN_MUX_COUNT];
uint8_t g_adc_scan_sweep_done;
adc_scan_column_entry_t *g_adc_scan_pending_entry;
uint16_t g_adc_scan_parallel_filtered[ADC_SCAN_MUX_COUNT];
uint16_t g_adc_scan_parallel_raw[ADC_SCAN_MUX_COUNT];
uint8_t g_adc_scan_mode;
uint8_t g_adc_scan_settle_cycles;

/* Object sizes. */
_Static_assert(sizeof(g_adc_scan_sweep_samples_left) == 1, "g_adc_scan_sweep_samples_left size");
_Static_assert(sizeof(g_adc_scan_mode_applied) == 1, "g_adc_scan_mode_applied size");
_Static_assert(sizeof(g_adc_scan_event_queue) == 360, "g_adc_scan_event_queue size");
_Static_assert(sizeof(g_adc_scan_event_head) == 1, "g_adc_scan_event_head size");
_Static_assert(sizeof(g_adc_scan_event_tail) == 1, "g_adc_scan_event_tail size");
_Static_assert(sizeof(g_adc_scan_event_scan_end) == 1, "g_adc_scan_event_scan_end size");
_Static_assert(sizeof(g_adc_scan_cycle_busy) == 1, "g_adc_scan_cycle_busy size");
_Static_assert(sizeof(g_adc_scan_mode_requested) == 1, "g_adc_scan_mode_requested size");
_Static_assert(sizeof(g_adc_scan_mode_change_pending) == 1, "g_adc_scan_mode_change_pending size");
_Static_assert(sizeof(g_adc_scan_dma_samples) == 4 * ADC_SCAN_MUX_COUNT, "g_adc_scan_dma_samples size");
_Static_assert(sizeof(g_adc_scan_settling) == 1, "g_adc_scan_settling size");
_Static_assert(sizeof(g_adc_scan_column_table) == 12 * ADC_SCAN_MUX_CHANNELS * ADC_SCAN_MUX_COUNT,
               "g_adc_scan_column_table size");
_Static_assert(sizeof(g_adc_scan_parallel_mode) == 1, "g_adc_scan_parallel_mode size");
_Static_assert(sizeof(g_adc_scan_calibrating) == 1, "g_adc_scan_calibrating size");
_Static_assert(sizeof(g_adc_scan_sweep_aux_flag) == 1, "g_adc_scan_sweep_aux_flag size");
_Static_assert(sizeof(g_adc_scan_mux_index) == 4, "g_adc_scan_mux_index size");
_Static_assert(sizeof(g_adc_scan_mux_channel) == 4, "g_adc_scan_mux_channel size");
_Static_assert(sizeof(g_adc_scan_sweep_parallel_phase) == 1, "g_adc_scan_sweep_parallel_phase size");
_Static_assert(sizeof(g_adc_scan_parallel_seq_ready) == 1, "g_adc_scan_parallel_seq_ready size");
_Static_assert(sizeof(g_adc_scan_parallel_mux_order) == ADC_SCAN_MUX_COUNT, "g_adc_scan_parallel_mux_order size");
_Static_assert(sizeof(g_adc_scan_sweep_done) == 1, "g_adc_scan_sweep_done size");
_Static_assert(sizeof(g_adc_scan_pending_entry) == 4, "g_adc_scan_pending_entry size");
_Static_assert(sizeof(g_adc_scan_parallel_filtered) == 2 * ADC_SCAN_MUX_COUNT, "g_adc_scan_parallel_filtered size");
_Static_assert(sizeof(g_adc_scan_parallel_raw) == 2 * ADC_SCAN_MUX_COUNT, "g_adc_scan_parallel_raw size");
_Static_assert(sizeof(g_adc_scan_mode) == 1, "g_adc_scan_mode size");
_Static_assert(sizeof(g_adc_scan_settle_cycles) == 1, "g_adc_scan_settle_cycles size");
