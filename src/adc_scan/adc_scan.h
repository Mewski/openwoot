/*
 * adc_scan.h - public functions of the adc_scan module: the scan state machine of the hall-sensor
 * scan (start-up sweep, single-input and parallel scanning in the conversion interrupt, start of
 * a scan cycle from the main loop) and the key event ring read by the main loop.
 *
 * Depends on: hal/hal_scan.h for the converter, multiplexer inputs and trigger (the conversion
 * interrupt is the call-back hal_scan_conversion_isr in adc_scan_irq.c); analog for the key
 * records.
 */
#ifndef ADC_SCAN_ADC_SCAN_H_
#define ADC_SCAN_ADC_SCAN_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "adc_scan/adc_scan_types.h"
#include "analog/analog_types.h"
#include "adc_scan/adc_scan_data.h"

/* Claim the next slot of the key event ring; returns its index, or -1 when the ring is full. */
int adc_scan_event_reserve(void);
/* Queue one key event (key id, key record, travel 0..4095); dropped when the ring is full. */
void adc_scan_event_push(uint8_t key_id, analog_key_state_t *key_state, uint16_t travel);
/* Take the oldest event out of the ring; returns an adc_scan_event_pop_result value. */
int adc_scan_event_pop(adc_scan_event_t *event_out);
/* One conversion of the start-up baseline sweep (called from the conversion interrupt). */
void adc_scan_sweep_step(void);
/* Regular scan, single-input mode: evaluate one conversion and trigger the next one. */
void adc_scan_process_single_sample(void);
/* Regular scan, parallel mode: evaluate the conversions of all inputs at one multiplexer address. */
void adc_scan_process_parallel_samples(void);
/* Switch the inputs and the converter to scan mode `mode` (enum adc_scan_mode) and load the
 * matching key calibration; returns 1. */
int adc_scan_set_mode(uint32_t mode);
/* Main loop: start the next scan cycle (mode switch, idle recalibration scheduling, first trigger). */
void adc_scan_start_cycle(void);

/* Start-up steps (adc_scan_init.c), registered with the start-up list. */
void adc_scan_hardware_init(void);
void adc_scan_column_table_init(void);
void adc_scan_start_sweep(void);

#endif /* ADC_SCAN_ADC_SCAN_H_ */
