/*
 * adc_scan_tables.h - data of the sensor scan: the order of the multiplexers when all are
 * converted together. Each table is described at its definition in adc_scan_tables.c.
 *
 * Read by the scan state machine (src/adc_scan/), which passes the order to the scan hardware
 * code of the family. The pins and converter channels of the scan are wiring (wiring.h).
 * Depends on board.h for the dimensions.
 */
#ifndef TABLES_ADC_SCAN_TABLES_H
#define TABLES_ADC_SCAN_TABLES_H

#include <stdint.h>

#include "board.h"

extern const uint8_t k_adc_scan_parallel_mux_order[BOARD_MUX_COUNT];

#endif /* TABLES_ADC_SCAN_TABLES_H */
