/*
 * adc_scan_magnitude.h - from a result word of the converter to the magnitude the analog module
 * works with: the distance of the sensor reading from the zero-field level, 0 within the dead
 * band. Internal to adc_scan; inline because the start-up sweep and both scan paths use it for
 * every sample.
 *
 * Depends on: hal/hal_scan.h (the reading of a word), feature reference (the level it is measured
 * against), analog (the dead band).
 */
#ifndef ADC_SCAN_ADC_SCAN_MAGNITUDE_H
#define ADC_SCAN_ADC_SCAN_MAGNITUDE_H

#include <stdint.h>

#include "adc_scan/reference/reference.h"
#include "analog/analog_types.h"
#include "hal/hal_scan.h"

/* |reading - level| of the result word `word`, 0 within +/-ANALOG_ADC_DEADBAND. */
static inline uint16_t adc_scan_magnitude(uint32_t word)
{
    uint32_t sample = hal_scan_sample_value(word);
    uint32_t reference = adc_scan_reference_level(word);

    if (sample - reference + ANALOG_ADC_DEADBAND <= 2u * ANALOG_ADC_DEADBAND) {
        return 0u;
    }
    return (uint16_t)(sample > reference ? sample - reference : reference - sample);
}

#endif /* ADC_SCAN_ADC_SCAN_MAGNITUDE_H */
