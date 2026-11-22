/*
 * reference.h - feature reference: the zero-field level of the sensors is measured with every
 * sample instead of assumed. The hardware converts a reference input at the same instant as each
 * sensor reading (hal_scan_sample_reference); a reading counts from that level. A board without
 * the feature counts every reading from BOARD_ADC_ZERO_LEVEL.
 *
 * The scan paths call these functions for every sample, so all of them are inline: a board
 * without the feature pays nothing.
 *
 * Depends on: hal/hal_scan.h (the reference carried by a result word), the board's
 * BOARD_ADC_ZERO_LEVEL.
 */
#ifndef ADC_SCAN_REFERENCE_REFERENCE_H
#define ADC_SCAN_REFERENCE_REFERENCE_H

#include <stdint.h>

#include "board.h"
#include "hal/hal_scan.h"

#if FEATURE_REFERENCE

/* Reference converted with the last reading of scan position 0 (in the start-up sweep: with the
 * last reading of any position); reported to the host by feature command 20. */
extern uint16_t g_adc_scan_ref_level;

/* The level the reading in `word` is measured against: the reference converted with it, or
 * BOARD_ADC_ZERO_LEVEL while the reference reads 0. */
static inline uint32_t adc_scan_reference_level(uint32_t word)
{
    uint32_t reference = hal_scan_sample_reference(word);

    return (reference != 0u) ? reference : (uint32_t)BOARD_ADC_ZERO_LEVEL;
}

/* Keeps the reference converted with `word` as the level reported to the host. */
static inline void adc_scan_reference_latch(uint32_t word)
{
    g_adc_scan_ref_level = hal_scan_sample_reference(word);
}

/* The kept reference in the 8-bit scale of feature command 20. */
static inline uint8_t adc_scan_reference_report(void)
{
    return (uint8_t)(g_adc_scan_ref_level >> 4);
}

#else

static inline uint32_t adc_scan_reference_level(uint32_t word)
{
    (void)word;
    return (uint32_t)BOARD_ADC_ZERO_LEVEL;
}

static inline void adc_scan_reference_latch(uint32_t word)
{
    (void)word;
}

static inline uint8_t adc_scan_reference_report(void)
{
    return 0u;
}

#endif /* FEATURE_REFERENCE */

#endif /* ADC_SCAN_REFERENCE_REFERENCE_H */
