/*
 * adc_scan_irq.c - the conversion interrupt (hal_scan_conversion_isr) and the start-up baseline
 * sweep.
 *
 * The sweep runs once after reset while g_adc_scan_calibrating is set (started by
 * adc_scan_start_sweep with a 254 us first trigger, multiplexer 0, address 0):
 *   phase 1: one input per conversion, every multiplexer x every address in use, 100 conversions each;
 *   phase 2: all inputs per trigger, every address in use, 100 triggers each.
 * Per key it measures the rest level (maximum) and the noise band of both scan modes and derives the
 * initial bottom-out level.
 *
 * Depends on: hal_scan; feature reference; analog (key records, switch parameters); layout (scan
 * position table); the board's parallel input order.
 */
#include <math.h>
#include <stddef.h>
#include <string.h>

#include "adc_scan/adc_scan.h"
#include "adc_scan/adc_scan_magnitude.h"
#include "adc_scan/reference/reference.h"
#include "analog/analog.h"
#include "hal/hal_scan.h"
#include "layout/layout_data.h"
#include "tables/adc_scan_tables.h"

#define ADC_SCAN_LAST_MUX_CHANNEL    (ADC_SCAN_MUX_CHANNELS - 1u)

#define ADC_SCAN_SWEEP_BAND_FACTOR   1.2f  /* noise band = half the measured spread, times this factor */
#define ADC_SCAN_SWEEP_PARALLEL_REST_MARGIN 5u /* added to the parallel rest level when its band is <= 4 */
#define ADC_SCAN_SWEEP_SMALL_BAND    4u
#define ANALOG_CURVE_ROUNDING_Q8     0x7FF80u /* ANALOG_CURVE_END_Q8_MAX / 2 */

/* Magnitude of one sweep sample; feature reference keeps the reference converted with it. */
static uint16_t adc_scan_sweep_sample_magnitude(uint32_t sample_word)
{
    adc_scan_reference_latch(sample_word);
    return adc_scan_magnitude(sample_word);
}

/* Last conversion of a position. Turns the tracked maximum / minimum into the stored calibration
 * of the current phase, restarts the trackers and resets the bottom-out estimate of the key (same
 * computation as the tail of analog_apply_switch_profiles). */
static void adc_scan_sweep_finish_key(analog_key_state_t *key, uint8_t parallel_phase)
{
    uint16_t rest = key->rest_level; /* running maximum */
    float spread = (float)rest - (float)key->noise_band; /* running minimum */
    uint16_t band = (uint16_t)(uint32_t)(ceilf(spread * 0.5f) * ADC_SCAN_SWEEP_BAND_FACTOR);

    if (parallel_phase != 0u) {
        key->noise_band_parallel = band;
        key->rest_level = 0u;
        key->noise_band = ANALOG_VALUE_NONE;
        key->rest_level_parallel = (band > ADC_SCAN_SWEEP_SMALL_BAND)
                                       ? rest
                                       : (uint16_t)(rest + ADC_SCAN_SWEEP_PARALLEL_REST_MARGIN);
        rest = key->rest_level_single;
    } else {
        key->noise_band_single = band;
        key->rest_level = 0u;
        key->noise_band = ANALOG_VALUE_NONE;
        key->rest_level_single = rest;
    }

    const analog_switch_params_t *sw =
        (key->has_switch_params != 0u) ? &key->switch_params : &g_analog_default_switch_params;

    /* Unsigned on purpose: a rest level above the nominal bottom-out level wraps. Matches stock
     * firmware 2.14.1. */
    uint32_t scaled = sw->curve_end_q8 * ((uint32_t)sw->nominal_bottom - rest) + ANALOG_CURVE_ROUNDING_Q8;
    uint32_t distance = scaled / ANALOG_CURVE_END_Q8_MAX;
    if (distance < ANALOG_MIN_BOTTOM_DISTANCE) {
        distance = ANALOG_MIN_BOTTOM_DISTANCE;
    }
    key->bottom_level = (uint16_t)(rest + distance);
    key->bottom_is_learned = 0u;

    memset(&key->bo_tracker, 0, sizeof(key->bo_tracker));
    key->bo_tracker.lower_bound = ANALOG_VALUE_NONE;
    key->bo_reference = ANALOG_VALUE_NONE;
    key->bo_estimate = ANALOG_VALUE_NONE;
}

/* Evaluates the conversion(s) of the current position: tracks maximum and minimum per key and
 * finishes the key on the last conversion. Returns the remaining conversions of the position
 * before this one is counted. */
static uint8_t adc_scan_sweep_track_samples(uint8_t parallel_phase)
{
    uint32_t input_count = (parallel_phase != 0u) ? ADC_SCAN_MUX_COUNT : 1u;
    analog_key_state_t *keys[ADC_SCAN_MUX_COUNT];
    uint16_t magnitudes[ADC_SCAN_MUX_COUNT];

    for (uint32_t input = 0u; input < input_count; input++) {
        uint32_t mux = (parallel_phase != 0u) ? g_adc_scan_parallel_mux_order[input]
                                              : (uint8_t)g_adc_scan_mux_index;
        uint8_t scan_pos = (uint8_t)(g_adc_scan_mux_channel + ADC_SCAN_POS_PER_MUX * mux);

        if (g_layout_scanpos_to_keyid[scan_pos] == ADC_SCAN_NO_KEY) {
            magnitudes[input] = 0u;
            keys[input] = NULL;
            continue;
        }
        keys[input] = &g_analog_key_states[scan_pos];
        magnitudes[input] = adc_scan_sweep_sample_magnitude(g_adc_scan_dma_samples[input]);
    }

    uint8_t samples_left = g_adc_scan_sweep_samples_left;

    for (uint32_t input = 0u; input < input_count; input++) {
        analog_key_state_t *key = keys[input];
        if (key == NULL) {
            continue;
        }

        /* rest_level tracks the maximum, noise_band the minimum (preset to 0xFFFF by analog_key_records_init) */
        uint16_t magnitude = magnitudes[input];
        if (key->rest_level < magnitude) {
            key->rest_level = magnitude;
        } else if (key->noise_band > magnitude) {
            key->noise_band = magnitude;
        }

        if (samples_left == 1u) {
            adc_scan_sweep_finish_key(key, parallel_phase);
        }
    }

    return samples_left;
}

/* End of phase 1: switch the converter, the result count and the multiplexer inputs to parallel
 * sampling (the inputs only if no earlier switch to parallel scanning has set them up). */
static void adc_scan_sweep_enter_parallel_phase(void)
{
    g_adc_scan_sweep_parallel_phase = 1u;

    if (g_adc_scan_parallel_seq_ready == 1u) {
        hal_scan_sweep_enter_parallel(NULL);
        return;
    }

    hal_scan_sweep_enter_parallel(k_adc_scan_parallel_mux_order);
    g_adc_scan_parallel_seq_ready = 1u;
    memcpy(g_adc_scan_parallel_mux_order, k_adc_scan_parallel_mux_order, sizeof(g_adc_scan_parallel_mux_order));
}

/* One step of the start-up baseline sweep, called from the conversion interrupt for every conversion
 * while g_adc_scan_calibrating is set: evaluates the sample(s), then repeats the position, moves to
 * the next address or multiplexer, or ends the sweep. */
void adc_scan_sweep_step(void)
{
    /* nothing to do outside the sweep */
    if (g_adc_scan_calibrating == 0u) {
        return;
    }

    uint8_t parallel_phase = g_adc_scan_sweep_parallel_phase;
    uint8_t samples_left;

    if (parallel_phase != 0u && g_adc_scan_parallel_seq_ready != 1u) {
        /* parallel sequence not configured yet: the conversion is only counted */
        samples_left = g_adc_scan_sweep_samples_left;
    } else {
        samples_left = adc_scan_sweep_track_samples(parallel_phase);
    }

    /* count the conversion; repeat the position until its 100 conversions are done */
    samples_left--;
    g_adc_scan_sweep_samples_left = samples_left;
    if (samples_left != 0u) {
        hal_scan_trigger_after(ADC_SCAN_DELAY_SWEEP_REPEAT_US);
        return;
    }

    /* position finished */
    g_adc_scan_sweep_samples_left = ADC_SCAN_SWEEP_SAMPLES;

    if (g_adc_scan_mux_channel < ADC_SCAN_LAST_MUX_CHANNEL) {
        uint32_t channel = g_adc_scan_mux_channel + 1u;
        g_adc_scan_mux_channel = channel;
        hal_scan_select_address(channel);

        uint8_t scan_pos = (uint8_t)(channel + ADC_SCAN_POS_PER_MUX * g_adc_scan_mux_index);
        if (parallel_phase == 0u && g_layout_scanpos_to_keyid[scan_pos] == ADC_SCAN_NO_KEY) {
            /* no sensor here: its 100 conversions still run, at the short interval */
            hal_scan_trigger_after(ADC_SCAN_DELAY_SAME_MUX_US);
        } else {
            hal_scan_trigger_after(ADC_SCAN_DELAY_ADDRESS_US);
        }
        return;
    }

    /* last address finished */
    g_adc_scan_mux_channel = 0u;
    hal_scan_select_address(0u);

    if (parallel_phase != 0u) {
        /* sweep finished; adc_scan_start_cycle takes over, no further trigger */
        g_adc_scan_calibrating = 0u;
        g_adc_scan_sweep_aux_flag = 0u;
        g_adc_scan_sweep_done = 1u;
        return;
    }

    /* phase 1: next multiplexer */
    uint32_t prev_mux = g_adc_scan_mux_index;
    uint32_t next_mux = (prev_mux + 1u) % ADC_SCAN_MUX_COUNT;
    hal_scan_select_input(prev_mux, next_mux);
    g_adc_scan_mux_index = next_mux;

    if (next_mux == 0u) {
        adc_scan_sweep_enter_parallel_phase();
    }
    hal_scan_trigger_after(ADC_SCAN_DELAY_MUX_US);
}

/* Conversion interrupt: acknowledges it and runs the start-up sweep or the scan path of the
 * current mode. */
void hal_scan_conversion_isr(void)
{
    hal_scan_ack_conversion();

    if (g_adc_scan_calibrating != 0u) {
        adc_scan_sweep_step();
    } else if (g_adc_scan_parallel_mode != 0u) {
        adc_scan_process_parallel_samples();
    } else {
        adc_scan_process_single_sample();
    }
}
