/*
 * adc_scan_tables.c - data of the sensor scan of this board (definitions; see
 * adc_scan_tables.h).
 */
#include "adc_scan_tables.h"

/* Multiplexer index per ADC sequence rank in parallel mode: 0..3 */
const uint8_t k_adc_scan_parallel_mux_order[BOARD_MUX_COUNT] = {
    0, 1, 2, 3,
};
