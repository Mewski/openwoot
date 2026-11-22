/*
 * adc_scan_sample.c - the two regular scan paths of the conversion interrupt.
 *
 * adc_scan_process_single_sample: one multiplexer input per conversion (scan mode 0).
 * adc_scan_process_parallel_samples: all inputs of one multiplexer address per trigger.
 *
 * Both convert the result word(s) to a magnitude (adc_scan_magnitude.h), run the per-key
 * hysteresis filter, select the next position and trigger its conversion, and only then run the
 * (identical) per-key processing that maintains the key record and queues events for the main
 * loop. Feature reference keeps the level of scan position 0 for the host; feature recheck may
 * have a changed reading converted a second time.
 *
 * Depends on: hal_scan, hal_time; features reference and recheck; analog (key records, noise
 * recalibration, bottom-out learning, rebound filter); core/clock.h (uptime) and
 * core/run_state.h (option flags); profile (rebound option).
 */
#include "adc_scan/adc_scan.h"
#include "adc_scan/adc_scan_magnitude.h"
#include "adc_scan/recheck/recheck.h"
#include "adc_scan/reference/reference.h"
#include "analog/analog.h"
#include "core/clock.h"
#include "core/run_state.h"
#include "hal/hal_scan.h"
#include "hal/hal_time.h"
#include "profile/profile_data.h"

#define ADC_SCAN_ACTIVITY_MARGIN     20u   /* value > rest threshold + 20 counts as key activity */
#define ANALOG_PLATEAU_LIMIT_NUM     229u  /* bottom_level <= 229 / 255 of the largest plateau */
#define ANALOG_PLATEAU_LIMIT_DEN     255u

/* ---- End of a cycle ------------------------------------------------------------------------------ */

/* End of a scan cycle: no further conversion is triggered, the stopwatch that places the first
 * conversion of the next cycle starts (hal_scan_end_cycle), and one ring slot is reserved as
 * end-of-cycle marker for adc_scan_event_pop. */
static void adc_scan_finish_cycle(void)
{
    hal_scan_end_cycle(ADC_SCAN_CYCLE_TIMER_US);

    int marker = adc_scan_event_reserve();
    if (marker == -1) {
        /* ring full: the newest event doubles as the marker */
        g_adc_scan_event_scan_end = g_adc_scan_event_head;
    } else {
        g_adc_scan_event_scan_end = (int8_t)marker;
    }
}

/* ---- Sample conversion -------------------------------------------------------------------------- */

/* Hysteresis filter: the filter output follows the magnitude at a distance of `band` and stays
 * where it is while the magnitude is closer. */
static inline uint16_t adc_scan_filter_hysteresis(uint16_t magnitude, uint16_t filtered, uint16_t band)
{
    int32_t diff = (int32_t)magnitude - (int32_t)filtered;
    if (diff > (int32_t)band) {
        return (uint16_t)(magnitude - band);
    }
    if (diff < -(int32_t)band) {
        return (uint16_t)(magnitude + band);
    }
    return filtered;
}

/* ---- Per-key processing (shared by both scan paths) --------------------------------------------- */

/* A calibrated key moved (or its noise statistics became unstable): hold the idle recalibration for
 * 5 s and cancel a running batch. */
static void adc_scan_note_key_activity(const analog_key_state_t *key)
{
    if (key->noise_band == 0u || key->rest_level == 0u) {
        return;
    }

    g_analog_recal_hold_stamp_ms = g_clock_uptime_ms;
    g_analog_recal_hold = 1u;

    if (g_analog_recal_running != 0u) {
        g_analog_recal_running = 0u;
        for (uint32_t i = 0u; i < ANALOG_KEY_COUNT; i++) {
            g_analog_key_states[i].recal_active = 0u;
        }
    }
}

/* Maintain the sensor value that counts as end of travel while the key is pressed. */
static void adc_scan_update_bottom_level(analog_key_state_t *key, uint16_t value)
{
    if (key->bottom_is_learned == 1u) {
        if (key->bo_estimate < ANALOG_VALUE_NONE) {
            key->bottom_level = key->bo_estimate;
            return;
        }
        key->bottom_is_learned = 0u;
    } else if (key->bottom_is_learned != 0u) {
        return;
    }

    /* not learned: keep the level at most 240 below the deepest value seen */
    if ((uint32_t)value > (uint32_t)key->bottom_level + ANALOG_BOTTOM_HEADROOM) {
        key->bottom_level = (uint16_t)(value - ANALOG_BOTTOM_HEADROOM);
    }

    if (key->bo_tracker.max_plateau == 0u) {
        return;
    }

    /* clamp to [rest_level, 229 / 255 of the largest accepted plateau] */
    uint32_t limit = ANALOG_PLATEAU_LIMIT_NUM * key->bo_tracker.max_plateau / ANALOG_PLATEAU_LIMIT_DEN;
    uint32_t level = key->bottom_level;
    if (level >= limit) {
        level = limit;
    }
    if (key->rest_level >= level) {
        level = key->rest_level;
    }
    key->bottom_level = (uint16_t)level;

    if (key->bo_tracker.confidence > ANALOG_BO_CONFIDENCE_ADOPT || key->bo_estimate < key->bottom_level) {
        key->bottom_level = key->bo_estimate;
        key->bottom_is_learned = 1u;
    }
}

/* A key at rest still produces events while one of its advanced keys has work to do or it waits for
 * its release. */
static bool adc_scan_key_needs_rest_events(const analog_key_state_t *key)
{
    for (uint32_t layer = 0u; layer < ANALOG_KEY_LAYERS; layer++) {
        if (key->ak_slot[layer].index != ANALOG_NO_ADVANCED_KEY && key->ak_slot[layer].active != 0u) {
            return true;
        }
    }
    return (key->flags2 & ANALOG_KEY_FLAG2_IGNORE_UNTIL_RELEASE) != 0u;
}

/* Per-key processing of one evaluated sample: raw-scanning shortcut, idle noise recalibration,
 * key activity, bottom-out learning, release rebound detection and the event for the main loop.
 * magnitude = unfiltered magnitude of the reading, value = output of the hysteresis filter. */
static void adc_scan_process_key_sample(const adc_scan_column_entry_t *entry, uint16_t magnitude, uint16_t value)
{
    analog_key_state_t *key = entry->key_state;

    if ((g_run_state_options & RUN_STATE_OPTION_RAW_SCANNING) != 0u) {
        key->value = magnitude;
        return;
    }

    uint16_t prev_value = key->value;
    key->value = value;

    /* idle noise recalibration: feed the statistics with the unfiltered magnitude */
    if (key->recal_active != 0u) {
        uint32_t stats_state = analog_noise_stats_add_sample(&key->noise_stats, magnitude);
        if (stats_state == ANALOG_NOISE_DONE) {
            analog_apply_noise_recalibration(key);
        } else if (stats_state == ANALOG_NOISE_UNSTABLE) {
            adc_scan_note_key_activity(key);
        }
    }

    if (g_adc_scan_settling != 0u) {
        return;
    }

    uint16_t rest_threshold = (uint16_t)(key->rest_level + ANALOG_REST_MARGIN);

    if ((uint32_t)value > (uint32_t)rest_threshold + ADC_SCAN_ACTIVITY_MARGIN) {
        adc_scan_note_key_activity(key);
    }

    int rebound = 0;

    if (value > rest_threshold) {
        /* pressed */
        uint16_t now_us = (uint16_t)hal_time_micros();

        if (prev_value <= rest_threshold && key->bo_tracker.last_sample_value != 0u) {
            /* rest -> pressed edge: re-seed the velocity tracking of the bottom-out learning */
            key->bo_tracker.last_sample_value = prev_value;
            key->bo_tracker.last_sample_us = now_us;
            key->bo_tracker.stroke_active = 0u;
        }
        analog_update_bottom_out_estimate(key, value, now_us);
        adc_scan_update_bottom_level(key, value);

        if (g_profile_global_option3_active != 0u) {
            rebound = analog_detect_release_rebound(&key->rebound, rest_threshold, key->bottom_level, value,
                                                    (uint16_t)hal_time_micros());
        }
    } else {
        /* at rest */
        if (g_profile_global_option3_active != 0u && key->rebound.state != ANALOG_REBOUND_IDLE) {
            rebound = analog_detect_release_rebound(&key->rebound, rest_threshold, key->bottom_level, value,
                                                    (uint16_t)hal_time_micros());
        }
        /* event only on the pressed -> rest edge or while something still needs the key */
        if (prev_value <= rest_threshold && !adc_scan_key_needs_rest_events(key)) {
            return;
        }
    }

    /* a detected release rebound is reported as travel 0 */
    uint16_t travel = (rebound != 0) ? 0u : analog_compute_key_travel(key);
    adc_scan_event_push(entry->key_id, key, travel);
}

/* ---- Single-input path --------------------------------------------------------------------------- */

/* Advance to the next position that has a key: the addresses in use of one multiplexer, then the
 * next multiplexer. Updates *mux and *channel and returns the column table entry.
 * The successor multiplexer is computed once, so a multiplexer without any key would not be skipped
 * (the loop would spin); every multiplexer carries keys in the shipped layouts. */
static adc_scan_column_entry_t *adc_scan_next_position(uint32_t *mux, uint32_t *channel)
{
    uint32_t next_mux = (*mux + 1u) % ADC_SCAN_MUX_COUNT;
    uint32_t ch = *channel;
    uint32_t m = *mux;

    for (;;) {
        ch = (ch + 1u) % ADC_SCAN_MUX_CHANNELS;
        if (ch == 0u) {
            m = next_mux;
            if (g_adc_scan_column_table[0][m].key_state != NULL) {
                break;
            }
        } else {
            uint8_t mux_index = (uint8_t)m;
            if (mux_index < ADC_SCAN_MUX_COUNT && g_adc_scan_column_table[ch][mux_index].key_state != NULL) {
                break;
            }
        }
    }

    *mux = m;
    *channel = ch;
    return &g_adc_scan_column_table[ch][(uint8_t)m];
}

/* Single-input scan path of the ADC interrupt: evaluates the conversion of the pending key, selects
 * and arms the next keyed position, then processes the key. Ends the cycle when the scan wraps to
 * multiplexer 0. */
void adc_scan_process_single_sample(void)
{
    uint32_t sample_word = g_adc_scan_dma_samples[0];
    /* key whose conversion just finished; NULL only for the very first conversion after reset */
    adc_scan_column_entry_t *converted = g_adc_scan_pending_entry;

    /* sample -> magnitude */
    if (converted != NULL && converted->scan_pos == 0u) {
        adc_scan_reference_latch(sample_word);
    }
    uint16_t magnitude = adc_scan_magnitude(sample_word);
    uint16_t value = 0u;
    uint32_t mux = g_adc_scan_mux_index;

    if (converted != NULL) {
        /* hysteresis filter */
        analog_key_state_t *key = converted->key_state;
        value = adc_scan_filter_hysteresis(magnitude, key->filtered, key->noise_band);

        if (adc_scan_recheck_single(mux, value, key)) {
            return;
        }
        key->filtered = value;
    }

    /* select the next keyed position */
    uint32_t channel = g_adc_scan_mux_channel;
    adc_scan_column_entry_t *next_entry = adc_scan_next_position(&mux, &channel);
    g_adc_scan_mux_channel = channel;
    hal_scan_select_address(channel);

    /* arm its conversion before the (longer) key processing */
    bool cycle_end = false;
    uint32_t prev_mux = g_adc_scan_mux_index;
    if (prev_mux == mux) {
        hal_scan_trigger_after(ADC_SCAN_DELAY_SAME_MUX_US);
    } else {
        hal_scan_select_input(prev_mux, mux);
        g_adc_scan_mux_index = mux;
        if (mux != 0u) {
            hal_scan_trigger_after(ADC_SCAN_DELAY_MUX_US);
        } else {
            /* wrapped to multiplexer 0: cycle complete, adc_scan_start_cycle arms the next one */
            cycle_end = true;
        }
    }

    /* key processing of the conversion that just finished */
    if (converted != NULL) {
        adc_scan_process_key_sample(converted, magnitude, value);
    }

    /* remember the key whose conversion is now in flight */
    g_adc_scan_pending_entry = next_entry;
    if (cycle_end) {
        adc_scan_finish_cycle();
    }
}

/* ---- Parallel path ------------------------------------------------------------------------------- */

/* Commit the filter state of one input and buffer its values for the per-key loop. */
static inline void adc_scan_parallel_store(uint32_t input, analog_key_state_t *key, uint16_t magnitude,
                                           uint16_t value)
{
    key->filtered = value;
    g_adc_scan_parallel_filtered[input] = value;
    g_adc_scan_parallel_raw[input] = magnitude;
}

/* Parallel scan path of the ADC interrupt: evaluates the conversions of all inputs at the current
 * multiplexer address (feature recheck may defer some of them to a second conversion), arms the
 * next address, then processes the keys. Ends the cycle when the address wraps to 0. */
void adc_scan_process_parallel_samples(void)
{
    uint32_t channel = g_adc_scan_mux_channel;
    adc_scan_column_entry_t *column = g_adc_scan_column_table[channel];

    if (!adc_scan_recheck_parallel_pending()) {
        /* first pass, every input */
        for (uint32_t input = 0u; input < ADC_SCAN_MUX_COUNT; input++) {
            analog_key_state_t *key = column[input].key_state;
            if (key == NULL) {
                continue;
            }
            uint32_t word = g_adc_scan_dma_samples[input];
            if (column[input].scan_pos == 0u) {
                adc_scan_reference_latch(word);
            }
            uint16_t magnitude = adc_scan_magnitude(word);
            uint16_t value = adc_scan_filter_hysteresis(magnitude, key->filtered, key->noise_band);

            if (adc_scan_recheck_parallel_defer(input, value, key)) {
                continue;
            }
            adc_scan_parallel_store(input, key, magnitude, value);
        }

        if (adc_scan_recheck_parallel_start()) {
            return;
        }
    } else {
        /* second conversion: the deferred inputs, taken unconditionally */
        for (uint32_t input = adc_scan_recheck_first_input(); input < ADC_SCAN_MUX_COUNT; input++) {
            analog_key_state_t *key = column[input].key_state;
            if (!adc_scan_recheck_parallel_flagged(input) || key == NULL) {
                continue;
            }
            uint16_t magnitude = adc_scan_magnitude(g_adc_scan_dma_samples[input]);
            uint16_t value = adc_scan_filter_hysteresis(magnitude, key->filtered, key->noise_band);
            adc_scan_parallel_store(input, key, magnitude, value);
        }
    }

    /* next address; the conversion is armed before the key processing */
    channel = (channel + 1u) % ADC_SCAN_MUX_CHANNELS;
    g_adc_scan_mux_channel = channel;
    adc_scan_recheck_reset();
    hal_scan_select_address(channel);

    bool cycle_end = (channel == 0u);
    if (!cycle_end) {
        hal_scan_trigger_after(ADC_SCAN_DELAY_SAME_MUX_US);
    }

    /* the keys of the address that was just converted */
    for (uint32_t input = 0u; input < ADC_SCAN_MUX_COUNT; input++) {
        if (column[input].key_state != NULL) {
            adc_scan_process_key_sample(&column[input], g_adc_scan_parallel_raw[input],
                                        g_adc_scan_parallel_filtered[input]);
        }
    }

    /* end of the cycle: reserve the end-of-cycle marker */
    if (cycle_end) {
        adc_scan_finish_cycle();
    }
}
