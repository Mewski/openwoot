/*
 * recheck.h - feature recheck: a reading of a converter input from BOARD_SCAN_RECHECK_FIRST_INPUT
 * on that would change the filtered value of its key is not trusted. The same position is
 * converted a second time ADC_SCAN_DELAY_SAME_MUX_US later and only that second result is used.
 *
 * Single-input scanning asks adc_scan_recheck_single once per reading. Parallel scanning converts
 * all inputs of an address at once: the first pass defers the changed readings
 * (adc_scan_recheck_parallel_defer), adc_scan_recheck_parallel_start converts the address again if
 * any was deferred, the second pass evaluates only the deferred inputs, which are all from
 * adc_scan_recheck_first_input on (adc_scan_recheck_parallel_pending,
 * adc_scan_recheck_parallel_flagged), and adc_scan_recheck_reset closes the address.
 *
 * The scan paths call these functions for every sample, so all of them are inline: a board
 * without the feature pays nothing.
 *
 * Depends on: hal/hal_scan.h (trigger), the adc_scan scan constants, the analog key record, the
 * board's BOARD_SCAN_RECHECK_FIRST_INPUT.
 */
#ifndef ADC_SCAN_RECHECK_RECHECK_H
#define ADC_SCAN_RECHECK_RECHECK_H

#include <stdbool.h>
#include <stdint.h>

#include "adc_scan/adc_scan_types.h"
#include "analog/analog_types.h"
#include "hal/hal_scan.h"

#if FEATURE_RECHECK

#define ADC_SCAN_RECHECK_FIRST_INPUT ((unsigned)BOARD_SCAN_RECHECK_FIRST_INPUT)

_Static_assert(BOARD_MUX_COUNT <= 8, "g_adc_scan_recheck_mask has one bit per converter input");

/* Single-input scan: 1 while the conversion in flight is the second one of a position. */
extern uint8_t g_adc_scan_recheck_pending;

/* Parallel scan: 1 while the current address is converted a second time. */
extern uint8_t g_adc_scan_recheck_pass;

/* Parallel scan: bit n = the reading of input n was deferred to the second conversion. */
extern uint8_t g_adc_scan_recheck_mask;

/* Single-input scan, reading `value` (after the hysteresis filter) of input `input` for `key`:
 * true if the reading is not trusted and the same position is being converted again; the caller
 * then drops the reading. A second conversion is always trusted. */
static inline bool adc_scan_recheck_single(uint32_t input, uint16_t value, const analog_key_state_t *key)
{
    if (input >= ADC_SCAN_RECHECK_FIRST_INPUT && g_adc_scan_recheck_pending == 0u && value != key->filtered) {
        hal_scan_trigger_after(ADC_SCAN_DELAY_SAME_MUX_US);
        g_adc_scan_recheck_pending = 1u;
        return true;
    }
    g_adc_scan_recheck_pending = 0u;
    return false;
}

/* Parallel scan, first pass: true if the reading `value` of input `input` for `key` is deferred
 * to the second conversion; the caller then leaves the key untouched. */
static inline bool adc_scan_recheck_parallel_defer(uint32_t input, uint16_t value, const analog_key_state_t *key)
{
    if (input >= ADC_SCAN_RECHECK_FIRST_INPUT && value != key->filtered) {
        g_adc_scan_recheck_mask |= (uint8_t)(1u << input);
        return true;
    }
    return false;
}

/* Parallel scan, end of the first pass: true if a reading was deferred and the address is being
 * converted again; the caller then ends the interrupt. */
static inline bool adc_scan_recheck_parallel_start(void)
{
    if (g_adc_scan_recheck_mask != 0u) {
        g_adc_scan_recheck_pass = 1u;
        hal_scan_trigger_after(ADC_SCAN_DELAY_SAME_MUX_US);
        return true;
    }
    return false;
}

/* Parallel scan: true while the results are those of the second conversion. */
static inline bool adc_scan_recheck_parallel_pending(void)
{
    return g_adc_scan_recheck_pass != 0u;
}

/* Parallel scan, second pass: the first input whose reading can have been deferred; the pass
 * looks at the inputs from this one on. */
static inline uint32_t adc_scan_recheck_first_input(void)
{
    return ADC_SCAN_RECHECK_FIRST_INPUT;
}

/* Parallel scan, second pass: true if the reading of input `input` was deferred. */
static inline bool adc_scan_recheck_parallel_flagged(uint32_t input)
{
    return (g_adc_scan_recheck_mask & (1u << input)) != 0u;
}

/* Parallel scan: the address is finished, the next one starts with a first pass. */
static inline void adc_scan_recheck_reset(void)
{
    g_adc_scan_recheck_mask = 0u;
    g_adc_scan_recheck_pass = 0u;
}

#else

static inline bool adc_scan_recheck_single(uint32_t input, uint16_t value, const analog_key_state_t *key)
{
    (void)input;
    (void)value;
    (void)key;
    return false;
}

static inline bool adc_scan_recheck_parallel_defer(uint32_t input, uint16_t value, const analog_key_state_t *key)
{
    (void)input;
    (void)value;
    (void)key;
    return false;
}

static inline bool adc_scan_recheck_parallel_start(void)
{
    return false;
}

static inline bool adc_scan_recheck_parallel_pending(void)
{
    return false;
}

/* No input is ever deferred: the second pass has no input to look at. */
static inline uint32_t adc_scan_recheck_first_input(void)
{
    return ADC_SCAN_MUX_COUNT;
}

static inline bool adc_scan_recheck_parallel_flagged(uint32_t input)
{
    (void)input;
    return false;
}

static inline void adc_scan_recheck_reset(void)
{
}

#endif /* FEATURE_RECHECK */

#endif /* ADC_SCAN_RECHECK_RECHECK_H */
