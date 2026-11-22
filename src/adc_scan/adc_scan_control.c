/*
 * adc_scan_control.c - main-loop side of the hall-sensor scan: scan mode switch (one input per
 * conversion <-> all inputs per trigger) and the start of a scan cycle together with the
 * scheduler of the idle noise recalibration.
 */
#include <string.h>

#include "adc_scan/adc_scan.h"
#include "analog/analog.h"
#include "core/clock.h"
#include "core/run_state.h"
#include "hal/hal_irq.h"
#include "hal/hal_scan.h"
#include "hal/hal_time.h"
#include "rgb/rgb_effects.h"
#include "tables/adc_scan_tables.h"
#include "tables/analog_tables.h"

/* hal_scan_trigger_at arguments of a regular cycle start: compare value = argument - 1, counted
 * from the end of the previous cycle (the stopwatch of hal_scan_end_cycle) */
#define ADC_SCAN_CYCLE_START_PARALLEL_TICKS 3u
#define ADC_SCAN_CYCLE_START_SINGLE_TICKS   55u

/* margins added to the stored parallel calibration for scan modes above 1 */
#define ADC_SCAN_FAST_REST_MARGIN    10u
#define ADC_SCAN_FAST_BAND_MARGIN    2u

#define ANALOG_RECAL_PATTERN_COUNT   4u    /* entries of k_analog_recal_batch_counts */
#define ANALOG_RECAL_TIME_SOURCE_UPTIME_MS 0u /* g_analog_recal_pass_time_source */
#define ANALOG_RECAL_TIME_SOURCE_TIMER_US  1u

/* ---- Scan mode ---------------------------------------------------------------------------------- */

/* Stop feeding the noise statistics of the idle recalibration. */
static void analog_recal_stop_sampling(void)
{
    if (g_analog_recal_running != 0u) {
        g_analog_recal_running = 0u;
        for (uint32_t i = 0u; i < ANALOG_KEY_COUNT; i++) {
            g_analog_key_states[i].recal_active = 0u;
        }
    }
}

/* Select how the idle recalibration groups the keys in the new scan mode. */
static void analog_recal_select_pattern(uint32_t mode)
{
    uint8_t pattern;

    if (mode == ADC_SCAN_MODE_SINGLE) {
        pattern = (g_analog_recal_done_single != 0u && g_analog_recal_done_parallel != 0u)
                      ? ANALOG_RECAL_PATTERN_COLUMN_SPLIT
                      : ANALOG_RECAL_PATTERN_MUX_PAIR;
    } else if (mode - 1u <= 3u && g_analog_recal_done_parallel == 0u) {
        pattern = ANALOG_RECAL_PATTERN_MUX_PAIR;
    } else if (mode == 1u) {
        pattern = ANALOG_RECAL_PATTERN_COLUMN_SPLIT;
    } else if (mode - 2u <= 2u) {
        pattern = ANALOG_RECAL_PATTERN_COLUMN;
    } else {
        pattern = ANALOG_RECAL_PATTERN_SINGLE_KEY;
    }
    g_analog_recal_pattern = pattern;
}

/* Scan mode changed: the key records get the stored calibration of the new mode and the idle
 * recalibration starts over. */
static void adc_scan_apply_mode_calibration(uint32_t mode, uint8_t prev_mode)
{
    analog_recal_stop_sampling();
    g_analog_recal_batch = 0u;
    g_analog_recal_pass_done = 0u;
    g_analog_recal_hold = 0u;

    analog_recal_select_pattern(mode);

    /* Leaving single-input mode: keep the active calibration of the keys recalibrated so far.
     * Never runs: g_analog_recal_batch was cleared above with interrupts masked. */
    uint8_t batch = g_analog_recal_batch;
    if (prev_mode == ADC_SCAN_MODE_SINGLE && batch != 0u) {
        for (uint32_t i = 0u; i < batch; i++) {
            g_analog_key_states[i].rest_level_single = g_analog_key_states[i].rest_level;
            g_analog_key_states[i].noise_band_single = g_analog_key_states[i].noise_band;
        }
    }

    for (uint32_t i = 0u; i < ANALOG_KEY_COUNT; i++) {
        analog_key_state_t *key = &g_analog_key_states[i];
        uint16_t noise_band;

        if (mode == ADC_SCAN_MODE_SINGLE) {
            key->rest_level = key->rest_level_single;
            noise_band = key->noise_band_single;
        } else if (mode <= 1u) {
            key->rest_level = key->rest_level_parallel;
            noise_band = key->noise_band_parallel;
        } else {
            key->rest_level = (uint16_t)(key->rest_level_parallel + ADC_SCAN_FAST_REST_MARGIN);
            noise_band = (uint16_t)(key->noise_band_parallel + ADC_SCAN_FAST_BAND_MARGIN);
        }
        key->value = 0u;
        key->filtered = 0u;
        key->sensor_state = 0u;
        key->noise_band = noise_band;
    }

    g_adc_scan_mode_applied = (uint8_t)mode;
}

/* Switch the scan to `mode` (enum adc_scan_mode; 0 = single input, 1..7 = all inputs per trigger):
 * multiplexer inputs, converter sequence, result count, and the key calibration of that mode.
 * Runs with interrupts masked. Always returns 1. */
int adc_scan_set_mode(uint32_t mode)
{
    uint32_t irq_state = hal_irq_save();

    g_adc_scan_mode = (uint8_t)mode;
    g_adc_scan_mux_channel = 0u;

    if (mode != ADC_SCAN_MODE_SINGLE) {
        /* all inputs per trigger: result n is the reading of input g_adc_scan_parallel_mux_order[n] */
        g_adc_scan_parallel_seq_ready = 0u;
        memcpy(g_adc_scan_parallel_mux_order, k_adc_scan_parallel_mux_order, sizeof(g_adc_scan_parallel_mux_order));
        hal_scan_enter_parallel(g_adc_scan_parallel_mux_order, g_adc_scan_mux_channel);
        g_adc_scan_parallel_seq_ready = 1u;
    } else {
        /* one input per conversion: multiplexer 0, address 0 */
        hal_scan_enter_single(g_adc_scan_mux_index);
        g_adc_scan_mux_index = 0u;
    }

    uint8_t prev_mode = g_adc_scan_mode_applied;
    g_adc_scan_parallel_mode = (mode != ADC_SCAN_MODE_SINGLE) ? 1u : 0u;
    if (mode != prev_mode) {
        adc_scan_apply_mode_calibration(mode, prev_mode);
    }

    rgb_effects_reset_key_state();

    hal_irq_restore(irq_state);
    return 1;
}

/* ---- Scan cycle start --------------------------------------------------------------------------- */

/* Empty the event ring and mark the cycle as running (the busy flag is stored as 0, then 1). */
static inline void adc_scan_event_queue_reset(void)
{
    g_adc_scan_event_tail = ADC_SCAN_EVENT_INDEX_NONE;
    g_adc_scan_event_head = ADC_SCAN_EVENT_INDEX_NONE;
    g_adc_scan_event_scan_end = ADC_SCAN_EVENT_INDEX_NONE;
    g_adc_scan_cycle_busy = 0u;
    g_adc_scan_cycle_busy = 1u;
}

/* Trigger the first conversion of a regular cycle. */
static inline void adc_scan_arm_cycle_start(void)
{
    if (g_adc_scan_parallel_mode != 0u) {
        hal_scan_trigger_at(ADC_SCAN_CYCLE_START_PARALLEL_TICKS);
    } else {
        hal_scan_trigger_at(ADC_SCAN_CYCLE_START_SINGLE_TICKS);
    }
}

/* Apply the requested scan mode; key evaluation is skipped for the next three cycles. */
static void adc_scan_switch_mode(void)
{
    g_adc_scan_settling = 1u;
    g_adc_scan_settle_cycles = ADC_SCAN_SETTLE_CYCLES;
    if (g_adc_scan_calibrating == 0u && adc_scan_set_mode(g_adc_scan_mode_requested) != 0) {
        g_adc_scan_mode_change_pending = 0u;
    }
    hal_scan_trigger_after(ADC_SCAN_DELAY_MUX_US);
}

/* Decides whether the idle recalibration may sample in this cycle; starts a new batch when nothing
 * is running. Returns false while it has to wait (5000 ms after key activity, 20000 ms after a
 * complete pass). */
static bool analog_recal_should_run(void)
{
    if (g_analog_recal_running != 0u) {
        return true;
    }

    if (g_analog_recal_hold != 0u) {
        if (g_clock_uptime_ms - g_analog_recal_hold_stamp_ms < ANALOG_RECAL_HOLD_MS) {
            if (g_analog_recal_pass_done != 0u &&
                g_clock_uptime_ms - g_analog_recal_pass_done_stamp_ms >= ANALOG_RECAL_PASS_PAUSE_MS) {
                g_analog_recal_pass_done = 0u;
            }
            return false;
        }
        g_analog_recal_hold = 0u;
    }

    if (g_analog_recal_pass_done != 0u) {
        if (g_clock_uptime_ms - g_analog_recal_pass_done_stamp_ms < ANALOG_RECAL_PASS_PAUSE_MS) {
            return false;
        }
        g_analog_recal_pass_done = 0u;
    }

    /* start the batch g_analog_recal_batch */
    g_analog_recal_hold_stamp_ms = g_clock_uptime_ms;
    analog_recal_reset_batch_stats();
    g_analog_recal_running = 1u;
    if (g_analog_recal_batch == 0u || g_analog_recal_pass_timing == 0u) {
        g_analog_recal_pass_start_ms = g_clock_uptime_ms;
        g_analog_recal_pass_time_source = ANALOG_RECAL_TIME_SOURCE_UPTIME_MS;
        g_analog_recal_pass_timing = 1u;
    }
    return true;
}

/* Walk the keys of the current batch, (re)start the statistics of keys that are idle or were
 * disturbed. Returns true while at least one key is still sampling. */
static bool analog_recal_poll_batch(uint8_t pattern)
{
    /* iterator set-up, same code as the head of analog_recal_reset_batch_stats */
    uint8_t batch = g_analog_recal_batch;
    analog_recal_iter_t iter = { .pos = 0u, .pattern = pattern, .base = 0u, .count = 0u };

    switch (pattern) {
    case ANALOG_RECAL_PATTERN_SINGLE_KEY:
        iter.base = batch;
        break;
    case ANALOG_RECAL_PATTERN_COLUMN_SPLIT:
        iter.pos = (uint8_t)(ADC_SCAN_MUX_COUNT * (batch / ADC_SCAN_MUX_CHANNELS));
        iter.base = (uint8_t)(batch % ADC_SCAN_MUX_CHANNELS);
        iter.count = ADC_SCAN_MUX_COUNT;
        break;
    case ANALOG_RECAL_PATTERN_COLUMN:
        iter.base = batch;
        iter.count = ADC_SCAN_MUX_COUNT;
        break;
    case ANALOG_RECAL_PATTERN_MUX_PAIR:
        iter.base = (uint8_t)(2u * batch);
        break;
    default:
        break;
    }

    bool any_sampling = false;
    adc_scan_column_entry_t *entry = NULL;

    while (analog_recal_batch_next(&iter, &entry) != 0) {
        if (entry == NULL) {
            continue;
        }
        analog_key_state_t *key = entry->key_state;

        switch (key->noise_stats.state) {
        case ANALOG_NOISE_SAMPLING:
            if (key->recal_active == 0u) {
                /* statistics armed but the interrupt does not feed them: restart */
                analog_noise_stats_restart(&key->noise_stats);
                key->recal_active = 1u;
            }
            any_sampling = true;
            break;
        case ANALOG_NOISE_IDLE:
        case ANALOG_NOISE_UNSTABLE:
            analog_noise_stats_restart(&key->noise_stats);
            key->recal_active = 1u;
            any_sampling = true;
            break;
        default:
            /* ANALOG_NOISE_DONE: the interrupt has applied the new calibration */
            break;
        }
    }
    return any_sampling;
}

/* All keys of the batch are done: next batch, or end of the pass after the last batch of the
 * pattern. */
static void analog_recal_advance(uint8_t pattern)
{
    g_analog_recal_batch = (uint8_t)(g_analog_recal_batch + 1u);

    if (pattern < ANALOG_RECAL_PATTERN_COUNT && k_analog_recal_batch_counts[pattern] > g_analog_recal_batch) {
        analog_recal_reset_batch_stats();
        return;
    }

    /* pass complete */
    uint8_t scan_level = g_run_state_flags >> RUN_STATE_SCAN_MODE_SHIFT;
    uint8_t time_source = g_analog_recal_pass_time_source;
    uint32_t end_time = 0u;

    g_analog_recal_batch = 0u;
    if (time_source == ANALOG_RECAL_TIME_SOURCE_UPTIME_MS) {
        end_time = g_clock_uptime_ms;
    } else if (time_source == ANALOG_RECAL_TIME_SOURCE_TIMER_US) {
        end_time = hal_time_micros();
    }
    g_analog_recal_pass_end_time = end_time;
    g_analog_recal_pass_timing = 0u;

    analog_recal_stop_sampling();

    g_analog_recal_pass_done_stamp_ms = g_clock_uptime_ms;
    g_analog_recal_pass_done = 1u;
    if (scan_level != 0u) {
        g_analog_recal_done_parallel = 1u;
    } else {
        g_analog_recal_done_single = 1u;
    }
    g_analog_recal_pattern = ANALOG_RECAL_PATTERN_COLUMN_SPLIT;
}

/* Main loop: start the next scan cycle. Waits for the start-up sweep, counts down the settling
 * phase after a mode switch, applies a pending mode change, lets the idle noise recalibration
 * advance, empties the event ring and arms the first conversion. */
void adc_scan_start_cycle(void)
{
    /* the start-up sweep owns the ADC until it is done */
    while (g_adc_scan_calibrating != 0u) {
    }

    if (g_adc_scan_sweep_done != 0u) {
        /* first cycle after the sweep: load the calibration of the requested mode */
        g_adc_scan_sweep_done = 0u;
        g_adc_scan_mode_change_pending = 1u;
    }

    /* settling phase after a mode switch */
    if (g_adc_scan_settling != 0u) {
        if (g_adc_scan_settle_cycles != 0u) {
            g_adc_scan_settle_cycles--;
        } else {
            g_adc_scan_settling = 0u;
        }
    }

    if (g_adc_scan_mode_change_pending != 0u) {
        adc_scan_event_queue_reset();
        adc_scan_switch_mode();
        return;
    }

    if (g_adc_scan_settling == 0u && analog_recal_should_run()) {
        uint8_t pattern = g_analog_recal_pattern;
        if (!analog_recal_poll_batch(pattern)) {
            analog_recal_advance(pattern);
        }

        adc_scan_event_queue_reset();
        /* re-tested: the flag is re-read after the calls above */
        if (g_adc_scan_mode_change_pending != 0u) {
            adc_scan_switch_mode();
            return;
        }
        adc_scan_arm_cycle_start();
        return;
    }

    adc_scan_event_queue_reset();
    adc_scan_arm_cycle_start();
}
