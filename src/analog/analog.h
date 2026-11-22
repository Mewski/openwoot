/* Public functions of the analog module: per-key calibration, sensor value to travel conversion,
 * switch profiles, bottom-out learning, release-rebound filter and the scheduling of the idle noise
 * recalibration. The noise statistics and their evaluation are the selected noise_recal variant
 * (analog/noise_recal/noise_recal.h).
 *
 * Depends on: adc_scan (scan column table), the settings schema, the noise_recal strategy. */
#ifndef ANALOG_ANALOG_H_
#define ANALOG_ANALOG_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "proto/settings.pb.h"
#include "adc_scan/adc_scan_types.h"
#include "analog/analog_types.h"
#include "analog/analog_data.h"
#include "analog/noise_recal/noise_recal.h"

/* Read a 16-bit table at a Q8 index (index_q8 >> 8, fraction in the low byte), linearly interpolated. */
uint16_t analog_lut_interpolate(const uint16_t *lut, uint32_t index_q8);
/* Recalibration batch iterator: returns 0 at the end of the batch, else 1 with *entry_out set to
 * the next column table entry (NULL for a position without key). */
int analog_recal_batch_next(analog_recal_iter_t *iter, adc_scan_column_entry_t **entry_out);
/* Restart the noise statistics of all keys of the current recalibration batch. */
void analog_recal_reset_batch_stats(void);
/* Bottom-out learning: feed one sample of a pressed key (ADC interrupt). */
void analog_update_bottom_out_estimate(analog_key_state_t *key, uint32_t value, uint16_t now_us);
/* Give every switch description of file "sw" that has no switch type the default type. */
void analog_fill_missing_switch_types(SwitchProfiles *sw);
/* Build the run-time switch parameters from a switch description of file "sw". */
void analog_build_switch_params(analog_switch_params_t *out_params, const SwitchParams *sw_params);
/* Read file "sw" and bring it to the current version; returns 1 on success. */
int analog_load_switch_profiles(SwitchProfiles *sw_profiles);
/* Apply g_analog_switch_profiles to the key records and derive the bottom-out levels. */
void analog_apply_switch_profiles(void);
/* Travel units (0..4095 scale) per 0.1 mm of the key at row / col; 112 for a position without key. */
uint32_t analog_get_key_travel_per_0p1mm(uint32_t row, uint32_t col);
/* Convert the current sensor value of a key to travel 0..4095. */
uint16_t analog_compute_key_travel(const analog_key_state_t *key);
/* Release-rebound filter: returns 1 while the reading is a bounce after a release, else 0. */
int analog_detect_release_rebound(analog_rebound_filter_t *filter, uint32_t rest_level, uint32_t bottom_level, uint32_t reading, uint16_t now_us);

#endif /* ANALOG_ANALOG_H_ */
