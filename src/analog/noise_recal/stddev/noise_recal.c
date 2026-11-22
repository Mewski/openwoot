/*
 * noise_recal.c - noise_recal variant stddev: the noise band follows the standard deviation of a
 * window of 1000 samples at rest, the rest level its mean. A window ends early, as unstable, when
 * a moving average of the samples leaves the running mean (the key starts to move) or when the
 * standard deviation keeps changing after the first 400 samples. Matches stock firmware 2.14.1 of
 * the boards that select it.
 *
 * Depends on: analog (key record); adc_scan (the current scan mode).
 */
#include <math.h>

#include "analog/noise_recal/noise_recal.h"
#include "adc_scan/adc_scan_data.h"

#define NOISE_EMA_KEEP           0.95f  /* weight of the old moving average per sample */
#define NOISE_EMA_NEW            (1.0f - NOISE_EMA_KEEP) /* weight of the new sample; as a float this is
                                                          * not the same value as the literal 0.05f */
#define NOISE_MOVEMENT_SAMPLES   25u    /* no movement test before this many samples */
#define NOISE_MOVEMENT_LIMIT     4.0f   /* |moving average - mean| above this: the key moves */
#define NOISE_WARMUP_SAMPLES     400u   /* no stability check before this many samples */
#define NOISE_CHECK_FIRST        500    /* first check point after this many samples ... */
#define NOISE_CHECK_STEP         100    /* ... then one every 100 samples */
#define NOISE_CHECKS_MAX         9u     /* check points only while checks <= 9 */
#define NOISE_DRIFT_LIMIT        0.5f   /* accumulated change of the standard deviation */
#define NOISE_BAND_PER_STDDEV    2.8    /* noise band = 2.8 standard deviations */
#define NOISE_BAND_NEW_WEIGHT    0.9    /* new band = 90 % measured, 10 % old band */
#define NOISE_REST_OFFSET        2u     /* rest level = ceil(mean) + 2 */

void analog_noise_stats_restart(analog_noise_stats_t *stats)
{
    stats->count = 0u;
    stats->mean = 0.0f;
    stats->sum_sq = 0.0f;
    stats->last_stddev = 0.0f;
    stats->drift = 0.0f;
    stats->checks = 0u;
    stats->ema = 0.0f;
    stats->state = ANALOG_NOISE_SAMPLING;
}

/* From sample 26 on, a moving average further than 4 from the running mean ends the window as
 * ANALOG_NOISE_UNSTABLE. After 400 samples the standard deviation is checked every 100 samples
 * from sample 500 on; an accumulated change above 0.5 ends the window as ANALOG_NOISE_UNSTABLE.
 * 1000 samples end it as ANALOG_NOISE_DONE. */
uint32_t analog_noise_stats_add_sample(analog_noise_stats_t *stats, uint32_t sample)
{
    if (stats->state != ANALOG_NOISE_SAMPLING) {
        return stats->state;
    }

    /* Welford running mean / sum of squared deviations. */
    float value = (float)sample;
    uint32_t count = (uint16_t)(stats->count + 1u);
    float delta = value - stats->mean;
    stats->count = (uint16_t)count;
    float mean = delta / (float)(int32_t)count + stats->mean;
    stats->mean = mean;
    float sum_sq = stats->sum_sq + delta * (value - mean);
    stats->sum_sq = sum_sq;

    if (count == 1u) {
        stats->ema = value;
        return ANALOG_NOISE_SAMPLING;
    }

    /* The sample term comes first: with multiply-add fusion the compiler then rounds the product
     * of the old average and fuses the sample product, which fixes the last bit of the result
     * (matches stock firmware 2.14.1). */
    float ema = value * NOISE_EMA_NEW + stats->ema * NOISE_EMA_KEEP;
    stats->ema = ema;

    if (count > NOISE_MOVEMENT_SAMPLES) {
        if (fabsf(ema - mean) > NOISE_MOVEMENT_LIMIT) {
            stats->state = ANALOG_NOISE_UNSTABLE;
            return ANALOG_NOISE_UNSTABLE;
        }

        uint32_t checks = stats->checks;
        if (count > NOISE_WARMUP_SAMPLES && (int32_t)count > NOISE_CHECK_STEP * (int32_t)checks + NOISE_CHECK_FIRST
            && checks <= NOISE_CHECKS_MAX) {
            /* Check point: the standard deviation must have settled. */
            float stddev = sqrtf(sum_sq / (float)(int32_t)(count - 1u));

            if (checks != 0u) {
                float drift = (stddev - stats->last_stddev) + stats->drift;

                stats->drift = drift;
                if (fabsf(drift) > NOISE_DRIFT_LIMIT) {
                    stats->state = ANALOG_NOISE_UNSTABLE;
                    return ANALOG_NOISE_UNSTABLE;
                }
            }
            stats->checks = (uint8_t)(checks + 1u);
            stats->last_stddev = stddev;
        }
    }

    if (count >= ANALOG_NOISE_SAMPLES_DONE) {
        stats->state = ANALOG_NOISE_DONE;
        return ANALOG_NOISE_DONE;
    }
    return stats->state;
}

/* New noise band: 90 % of 2.8 standard deviations of the window and 10 % of the old band,
 * computed in double precision; new rest level: the mean rounded up, plus 2. */
void analog_apply_noise_recalibration(analog_key_state_t *key)
{
    uint32_t state = key->noise_stats.state;

    key->recal_active = 0u;
    if (state != ANALOG_NOISE_DONE) {
        return;
    }

    uint32_t count = key->noise_stats.count;
    float mean = key->noise_stats.mean;
    double measured = 0.0;
    if (count > 1u) {
        float stddev = sqrtf(key->noise_stats.sum_sq / (float)(int32_t)(count - 1u));
        measured = (double)(float)(stddev * NOISE_BAND_PER_STDDEV) * NOISE_BAND_NEW_WEIGHT;
    }
    float noise = (float)((float)key->noise_band * (1.0 - NOISE_BAND_NEW_WEIGHT) + measured);

    uint16_t noise_band = (uint16_t)(uint32_t)ceilf(noise);
    key->noise_band = noise_band;
    uint16_t rest_level = (uint16_t)((uint32_t)ceilf(mean) + NOISE_REST_OFFSET);
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
