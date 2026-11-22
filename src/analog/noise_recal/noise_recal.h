/*
 * noise_recal.h - interface of the strategy slot noise_recal: how the noise of a key at rest is
 * measured and turned into its rest level and noise band (the idle recalibration). Which keys
 * sample when is decided by the analog module (analog_recal.c) and the scan cycle start
 * (adc_scan_start_cycle); a variant owns the statistics of one key and their evaluation.
 *
 * The board selects one variant with STRATEGY_noise_recal in its board.mk. The variant's
 * noise_stats.h defines analog_noise_stats_t, the 28-byte statistics record inside the key
 * record; its noise_recal.c implements the three functions below.
 *
 * Depends on: analog (key record); adc_scan (the scan mode a new calibration belongs to).
 */
#ifndef ANALOG_NOISE_RECAL_NOISE_RECAL_H
#define ANALOG_NOISE_RECAL_NOISE_RECAL_H

#include <stdint.h>

#include "analog/analog_types.h"

/* analog_noise_stats_t.state; also the return value of analog_noise_stats_add_sample. */
enum analog_noise_stats_state {
    ANALOG_NOISE_IDLE     = 0,  /* record idle; re-armed by adc_scan_start_cycle */
    ANALOG_NOISE_SAMPLING = 1,  /* collecting (analog_noise_stats_restart) */
    ANALOG_NOISE_DONE     = 2,  /* window complete -> analog_apply_noise_recalibration */
    ANALOG_NOISE_UNSTABLE = 3,  /* the key moved during the window, window discarded */
};

#define ANALOG_NOISE_SAMPLES_DONE    1000u   /* samples that complete a window */

/* Empty the statistics of a key and start collecting (state ANALOG_NOISE_SAMPLING). */
void analog_noise_stats_restart(analog_noise_stats_t *stats);

/* Add one unfiltered magnitude of a key at rest (conversion interrupt); returns the state of the
 * statistics (enum analog_noise_stats_state). Does nothing unless the state is
 * ANALOG_NOISE_SAMPLING. */
uint32_t analog_noise_stats_add_sample(analog_noise_stats_t *stats, uint32_t sample);

/* The window of the key is complete: stop sampling the key and, if the state is
 * ANALOG_NOISE_DONE, set its rest level and noise band and keep them for the current scan mode
 * (g_adc_scan_mode 0: the single-input pair, 1: the parallel pair, above 1: not kept). */
void analog_apply_noise_recalibration(analog_key_state_t *key);

#endif /* ANALOG_NOISE_RECAL_NOISE_RECAL_H */
