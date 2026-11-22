/*
 * noise_stats.h - statistics record of the noise_recal variant stddev: running mean and variance
 * of the window, the drift of the standard deviation, and a moving average that detects a key
 * that starts to move.
 *
 * Depends on: nothing. Included by analog/analog_types.h, which places the record in the key
 * record.
 */
#ifndef ANALOG_NOISE_RECAL_NOISE_STATS_H
#define ANALOG_NOISE_RECAL_NOISE_STATS_H

#include <stddef.h>
#include <stdint.h>

/* 28 bytes at analog_key_state_t +0x5C (Welford running mean / variance). Offsets in the
 * comments are record offsets. */
typedef struct analog_noise_stats {
    float    mean;         /* +0x5C running mean */
    float    sum_sq;       /* +0x60 running sum of squared deviations */
    float    last_stddev;  /* +0x64 standard deviation at the previous check point */
    float    drift;        /* +0x68 accumulated change of the standard deviation */
    uint8_t  checks;       /* +0x6C check points done, 0..10; check point when count > 500 + 100 * checks */
    uint8_t  _pad_6d[3];
    float    ema;          /* +0x70 moving average of the samples, weight 0.05 per sample */
    uint16_t count;        /* +0x74 samples so far */
    uint8_t  state;        /* +0x76 enum analog_noise_stats_state */
    uint8_t  _pad_77;
} analog_noise_stats_t;

_Static_assert(sizeof(analog_noise_stats_t) == 0x1C, "analog_noise_stats_t size");
_Static_assert(offsetof(analog_noise_stats_t, mean) == 0x00, "noise_stats.mean");
_Static_assert(offsetof(analog_noise_stats_t, sum_sq) == 0x04, "noise_stats.sum_sq");
_Static_assert(offsetof(analog_noise_stats_t, last_stddev) == 0x08, "noise_stats.last_stddev");
_Static_assert(offsetof(analog_noise_stats_t, drift) == 0x0C, "noise_stats.drift");
_Static_assert(offsetof(analog_noise_stats_t, checks) == 0x10, "noise_stats.checks");
_Static_assert(offsetof(analog_noise_stats_t, ema) == 0x14, "noise_stats.ema");
_Static_assert(offsetof(analog_noise_stats_t, count) == 0x18, "noise_stats.count");
_Static_assert(offsetof(analog_noise_stats_t, state) == 0x1A, "noise_stats.state");

#endif /* ANALOG_NOISE_RECAL_NOISE_STATS_H */
