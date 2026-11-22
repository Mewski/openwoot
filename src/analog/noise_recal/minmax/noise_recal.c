/*
 * noise_recal.c - noise_recal variant minmax: the noise band follows the spread between the
 * smallest and the largest magnitude of a window of 1000 samples at rest, the rest level the
 * smallest magnitude plus that band. A window ends early, as unstable, when the standard deviation
 * keeps changing after the first 400 samples.
 *
 * Depends on: analog (key record); adc_scan (the current scan mode).
 */
#include <math.h>

#include "analog/noise_recal/noise_recal.h"
#include "adc_scan/adc_scan_data.h"

#define NOISE_STATS_MIN_RESET         0x0FFFu /* above every magnitude */
#define ANALOG_NOISE_WARMUP_SAMPLES   400u  /* no stability check before this many samples */
#define ANALOG_NOISE_CHECK_FIRST      500   /* first check point after this many samples ... */
#define ANALOG_NOISE_CHECK_STEP       100   /* ... then one every 100 samples */
#define ANALOG_NOISE_CHECKS_MAX       9u    /* check points only while checks <= 9 */
#define ANALOG_NOISE_DRIFT_LIMIT      0.5f  /* accumulated change of the standard deviation */

void analog_noise_stats_restart(analog_noise_stats_t *stats)
{
    stats->count = 0u;
    stats->max = 0u;
    stats->min = NOISE_STATS_MIN_RESET;
    stats->mean = 0.0f;
    stats->sum_sq = 0.0f;
    stats->last_stddev = 0.0f;
    stats->drift = 0.0f;
    stats->checks = 0u;
    stats->state = ANALOG_NOISE_SAMPLING;
}

/* After 400 samples the standard deviation is checked every 100 samples from sample 500 on; an
 * accumulated change above 0.5 ends the window as ANALOG_NOISE_UNSTABLE, 1000 samples end it as
 * ANALOG_NOISE_DONE. */
uint32_t analog_noise_stats_add_sample(analog_noise_stats_t *stats, uint32_t sample)
{
    float value;
    float delta;
    float mean;
    float sum_sq;
    float stddev;
    int32_t count;
    uint32_t checks;

    if (stats->state != ANALOG_NOISE_SAMPLING) {
        return stats->state;
    }

    value = (float)sample;
    if (value > (float)stats->max) {
        stats->max = (uint16_t)sample;
    } else if (value < (float)stats->min) {
        stats->min = (uint16_t)sample;
    }

    /* Welford running mean / sum of squared deviations. */
    count = (uint16_t)(stats->count + 1u);
    stats->count = (uint16_t)count;
    delta = value - stats->mean;
    sum_sq = stats->sum_sq;
    mean = delta / (float)count + stats->mean;
    stats->mean = mean;
    sum_sq = sum_sq + delta * (value - mean);
    stats->sum_sq = sum_sq;

    if ((uint32_t)count <= ANALOG_NOISE_WARMUP_SAMPLES) {
        return ANALOG_NOISE_SAMPLING;
    }

    checks = stats->checks;
    if (count <= ANALOG_NOISE_CHECK_STEP * (int32_t)checks + ANALOG_NOISE_CHECK_FIRST
        || checks > ANALOG_NOISE_CHECKS_MAX) {
        if ((uint32_t)count >= ANALOG_NOISE_SAMPLES_DONE) {
            stats->state = ANALOG_NOISE_DONE;
            return ANALOG_NOISE_DONE;
        }
        return stats->state;
    }

    /* Check point: the standard deviation must have settled. */
    stddev = sqrtf(sum_sq / (float)(count - 1));
    if (checks != 0u) {
        float drift = (stddev - stats->last_stddev) + stats->drift;

        stats->drift = drift;
        if (fabsf(drift) > ANALOG_NOISE_DRIFT_LIMIT) {
            stats->state = ANALOG_NOISE_UNSTABLE;
            return ANALOG_NOISE_UNSTABLE;
        }
    }
    stats->checks = (uint8_t)(checks + 1u);
    if (stats->count >= ANALOG_NOISE_SAMPLES_DONE) {
        stats->state = ANALOG_NOISE_DONE;
    }
    stats->last_stddev = stddev;
    return stats->state;
}

/* New noise band: 90 % of 0.52 * (max - min) and 10 % of the old band; new rest level: the minimum
 * plus the new band plus ANALOG_REST_MARGIN. */
void analog_apply_noise_recalibration(analog_key_state_t *key)
{
    uint32_t state = key->noise_stats.state;
    uint32_t count;
    int32_t min;
    float half_range;
    float noise;
    uint16_t noise_band;
    uint16_t rest_level;

    key->recal_active = 0u;
    if (state != ANALOG_NOISE_DONE) {
        return;
    }

    /* The standard deviation of the window is computed and not used. */
    count = key->noise_stats.count;
    if (count > 1u) {
        (void)sqrtf(key->noise_stats.sum_sq / (float)(int32_t)(count - 1u));
    }

    /* Computed in double precision. */
    min = key->noise_stats.min;
    half_range = (float)((float)(key->noise_stats.max - min) * 0.52);
    noise = (float)(half_range * 0.9 + (float)key->noise_band * (1.0 - 0.9));

    noise_band = (uint16_t)(uint32_t)ceilf(noise);
    key->noise_band = noise_band;
    rest_level = (uint16_t)((uint32_t)ceilf((float)min + noise) + ANALOG_REST_MARGIN);
    key->rest_level = rest_level;

    /* Keep the pair for the current scan mode (adc_scan_set_mode restores it). */
    if (g_adc_scan_mode > 1u) {
        return;
    }
    if (g_adc_scan_mode != 0u) {
        key->rest_level_parallel = rest_level;
        key->noise_band_parallel = noise_band;
    } else {
        key->rest_level_single = rest_level;
        key->noise_band_single = noise_band;
    }
}
