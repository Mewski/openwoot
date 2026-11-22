/*
 * adc_scan_init.c - the start-up steps of the hall-sensor scan: converter hardware,
 * microsecond clock, scan column table, start-up sweep and the first scan cycle, each
 * registered in its slot of the start-up list (core/startup.h).
 *
 * Also the call-back hal_scan_wait_ms, with which hal_scan_init waits for the converter.
 *
 * Depends on: hal_scan, hal_time; the layout's scan position table and the analog key
 * records; core/clock.h (the busy wait).
 */
#include "adc_scan/adc_scan.h"

#include "analog/analog_data.h"
#include "core/clock.h"
#include "core/startup.h"
#include "hal/hal_scan.h"
#include "hal/hal_time.h"
#include "layout/layout_data.h"

STARTUP_STEP(STARTUP_SCAN_HARDWARE, adc_scan_hardware_init);
STARTUP_STEP(STARTUP_TIME_MICROS, hal_time_start_micros);
STARTUP_STEP(STARTUP_SCAN_COLUMN_TABLE, adc_scan_column_table_init);
STARTUP_STEP(STARTUP_SCAN_SWEEP, adc_scan_start_sweep);
STARTUP_STEP(STARTUP_SCAN_CYCLE, adc_scan_start_cycle);

void hal_scan_wait_ms(uint32_t ms)
{
    clock_delay_ms(ms);
}

/*
 * Multiplexer inputs, address lines, converter, trigger and result transport into
 * g_adc_scan_dma_samples; the conversion interrupt is enabled, no conversion runs yet.
 */
void adc_scan_hardware_init(void)
{
    hal_scan_init(g_adc_scan_dma_samples);
}

/*
 * g_adc_scan_column_table[mux address][mux index]: key id and key record of the sensor
 * at scan position address + 16 * index, from the layout's scan position table.
 * Positions without a key get a NULL record; their key_id and scan_pos stay untouched
 * (zero from .bss).
 */
void adc_scan_column_table_init(void)
{
    const uint8_t *scanpos_to_keyid = g_layout_scanpos_to_keyid;

    for (uint32_t channel = 0; channel < ADC_SCAN_MUX_CHANNELS; channel++) {
        for (uint32_t mux = 0; mux < ADC_SCAN_MUX_COUNT; mux++) {
            adc_scan_column_entry_t *entry = &g_adc_scan_column_table[channel][mux];
            uint8_t scan_pos = (uint8_t)(channel + ADC_SCAN_POS_PER_MUX * mux);
            uint8_t key_id = scanpos_to_keyid[scan_pos];

            if (key_id != ADC_SCAN_NO_KEY) {
                entry->scan_pos = scan_pos;
                entry->key_state = &g_analog_key_states[scan_pos];
                entry->key_id = key_id;
            } else {
                entry->key_state = NULL;
            }
        }
    }
}

/*
 * Starts the start-up sweep: from now on the conversion interrupt measures the rest level
 * and noise of every sensor (adc_scan_sweep_step) while the start-up goes on with the LED
 * driver and USB; adc_scan_start_cycle waits for its end. The first conversion is
 * triggered ADC_SCAN_DELAY_FIRST_US after the timer start: multiplexer 0, address 0.
 */
void adc_scan_start_sweep(void)
{
    uint32_t previous_mux;

    if (g_adc_scan_parallel_mode != 0 && g_adc_scan_calibrating == 0) {
        adc_scan_set_mode(ADC_SCAN_MODE_SINGLE);
    }
    g_adc_scan_calibrating = 1;

    previous_mux = g_adc_scan_mux_index;
    (void)g_adc_scan_sweep_aux_flag;            /* loaded and discarded */
    hal_scan_select_input(previous_mux, 0);
    hal_scan_select_address(0);

    g_adc_scan_mux_index = 0;
    g_adc_scan_mux_channel = 0;
    hal_scan_trigger_after(ADC_SCAN_DELAY_FIRST_US);
}
