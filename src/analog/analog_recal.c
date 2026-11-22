/*
 * analog_recal.c - batches of the idle noise recalibration: the iterator over the keys of one
 * batch in the scan column table and the restart of their noise statistics. What the statistics
 * hold and how they become a calibration is the noise_recal strategy.
 *
 * Depends on: adc_scan (scan column table, scan geometry), the noise_recal strategy.
 */
#include "analog/analog.h"
#include "adc_scan/adc_scan_data.h"

#define ANALOG_RECAL_COLUMN_KEYS      ADC_SCAN_MUX_COUNT             /* keys of one mux address (patterns 1, 2) */
#define ANALOG_RECAL_MUX_PAIR_KEYS    (2u * ADC_SCAN_MUX_CHANNELS)   /* positions of two multiplexers (pattern 3) */

/* Batch iterator of the idle recalibration. Returns 0 when the batch is exhausted; otherwise 1,
 * advances the iterator and stores the column table entry of the next position in *entry_out
 * (NULL when the position carries no key). */
int analog_recal_batch_next(analog_recal_iter_t *iter, adc_scan_column_entry_t **entry_out)
{
    adc_scan_column_entry_t *entry = NULL;
    uint32_t pos = iter->pos;

    switch (iter->pattern) {
    case ANALOG_RECAL_PATTERN_SINGLE_KEY: {
        uint32_t channel = iter->base % ADC_SCAN_POS_PER_MUX;
        uint32_t mux = iter->base / ADC_SCAN_POS_PER_MUX;

        if (pos != 0u) {
            return 0;
        }
        iter->pos = 1u;
        if (channel < ADC_SCAN_MUX_CHANNELS && mux < ADC_SCAN_MUX_COUNT
            && g_adc_scan_column_table[channel][mux].key_state != NULL) {
            entry = &g_adc_scan_column_table[channel][mux];
        }
        break;
    }

    case ANALOG_RECAL_PATTERN_COLUMN_SPLIT:
    case ANALOG_RECAL_PATTERN_COLUMN: {
        uint32_t channel = iter->base;

        if (iter->count <= pos) {
            return 0;
        }
        if (channel < ADC_SCAN_MUX_CHANNELS && pos < ADC_SCAN_MUX_COUNT
            && g_adc_scan_column_table[channel][pos].key_state != NULL) {
            entry = &g_adc_scan_column_table[channel][pos];
        }
        iter->pos = (uint8_t)(pos + 1u);
        break;
    }

    case ANALOG_RECAL_PATTERN_MUX_PAIR: {
        uint32_t mux;
        uint32_t channel;

        if (pos >= ANALOG_RECAL_MUX_PAIR_KEYS) {
            return 0;
        }
        mux = (uint8_t)(iter->base + pos / ADC_SCAN_MUX_CHANNELS);
        if (mux >= ADC_SCAN_MUX_COUNT) {
            return 0;
        }
        channel = pos % ADC_SCAN_MUX_CHANNELS;
        iter->pos = (uint8_t)(pos + 1u);
        if (g_adc_scan_column_table[channel][mux].key_state != NULL) {
            entry = &g_adc_scan_column_table[channel][mux];
        }
        break;
    }

    default:
        return 0;
    }

    *entry_out = entry;
    return 1;
}

/* Restart the noise statistics of every key of the batch g_analog_recal_batch (grouping given by
 * g_analog_recal_pattern) and set them to ANALOG_NOISE_SAMPLING. */
void analog_recal_reset_batch_stats(void)
{
    adc_scan_column_entry_t *entry = NULL;
    analog_recal_iter_t iter;
    uint8_t batch = g_analog_recal_batch;

    iter.pattern = g_analog_recal_pattern;
    switch (g_analog_recal_pattern) {
    case ANALOG_RECAL_PATTERN_SINGLE_KEY:
        iter.pos = 0u;
        iter.base = batch;
        iter.count = 0u;
        break;
    case ANALOG_RECAL_PATTERN_COLUMN_SPLIT:
        iter.pos = (uint8_t)(ANALOG_RECAL_COLUMN_KEYS * (batch / ADC_SCAN_MUX_CHANNELS));
        iter.base = (uint8_t)(batch % ADC_SCAN_MUX_CHANNELS);
        iter.count = ANALOG_RECAL_COLUMN_KEYS;
        break;
    case ANALOG_RECAL_PATTERN_COLUMN:
        iter.pos = 0u;
        iter.base = batch;
        iter.count = ANALOG_RECAL_COLUMN_KEYS;
        break;
    case ANALOG_RECAL_PATTERN_MUX_PAIR:
        iter.pos = 0u;
        iter.base = (uint8_t)(2u * batch);
        iter.count = 0u;
        break;
    default:
        iter.pos = 0u;
        iter.base = 0u;
        iter.count = 0u;
        break;
    }

    while (analog_recal_batch_next(&iter, &entry) != 0) {
        if (entry != NULL) {
            analog_noise_stats_restart(&entry->key_state->noise_stats);
        }
    }
}
