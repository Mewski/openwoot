/*
 * app: the periodic work of the main loop: the 1 ms tick call-back, the two task bodies
 * (USB active / USB suspended) and the host-protocol command queue.
 *
 * Everything here except hal_time_tick_isr runs in main-loop context. The USB device task
 * (tud_task) is re-entered from the blocking waits; tud_task() returns at once before
 * tud_init().
 *
 * Depends on: hal_time, hal_irq, the LED matrix driver class, TinyUSB, core/clock.h,
 * core/run_state.h, core/startup.h, and the adc_scan, keys, profile, protocol and rgb
 * modules whose work the tasks run.
 */
#include "drivers/led_matrix.h"
#include "hal/hal_irq.h"
#include "hal/hal_time.h"
#include "tusb.h"

#include "adc_scan/adc_scan.h"
#include "app/tasks.h"
#include "core/clock.h"
#include "core/run_state.h"
#include "core/startup.h"
#include "keys/keys_data.h"
#include "keys/keys_scan_pass.h"
#include "profile/profile.h"
#include "protocol/protocol.h"
#include "rgb/rgb_frame.h"
#include "rgb/signals/signals.h"
#include "tables/keys_tables.h"
#include "usb/usb_types.h"

/* 1 ms idle passes between two scan passes while USB is suspended; also the initial value
 * of the pass counter, so that the first suspended pass scans. */
#define TASKS_SUSPEND_PASS_COUNT        100u

/* Pass counter of startup_run_suspended_task: counts 1 ms idle passes up to
 * TASKS_SUSPEND_PASS_COUNT, then one scan + key pass runs and it restarts at 0. */
static uint8_t s_suspend_pass_counter = TASKS_SUSPEND_PASS_COUNT;

/* ---- tick ----------------------------------------------------------------------------------- */

STARTUP_STEP(STARTUP_TIME_TICK, hal_time_start_tick);

/*
 * 1 ms tick (hal_time_start_tick). g_clock_wait_ms serves the busy waits,
 * g_clock_uptime_ms every timestamp and period of the firmware.
 */
void hal_time_tick_isr(void)
{
    g_clock_wait_ms++;
    g_clock_uptime_ms++;
}

/* ---- tasks ---------------------------------------------------------------------------------- */

/* LED driver chip shut down (rgb_wake_led_driver starts it again). */
static void startup_led_driver_shutdown(void)
{
    led_matrix_set_running(false);
}

/*
 * Follows a change of g_rgb_driver_enabled (rgb_update_output_enable:
 * USB configured and not suspended): wake the LED driver with a fade-in, or shut it
 * down. */
static void startup_apply_led_driver_state(void)
{
    if (g_rgb_driver_state_changed == 0) {
        return;
    }
    g_rgb_driver_state_changed = 0;

    if (g_rgb_driver_enabled != 0) {
        rgb_wake_led_driver();
    } else {
        startup_led_driver_shutdown();
    }
}

/*
 * Main-loop body while the bus is suspended (g_usb_link_flags bit 6). 100 passes only
 * wait for the next millisecond; then (or at once while a scan cycle is still running)
 * one scan + key pass runs, i.e. the keys are polled about every 100 ms. A key press
 * found by that pass wakes the host if it allowed remote wakeup.
 */
void startup_run_suspended_task(void)
{
    if (s_suspend_pass_counter < TASKS_SUSPEND_PASS_COUNT && g_adc_scan_cycle_busy == 0) {
        s_suspend_pass_counter++;
        clock_delay_ms(1);
        return;
    }
    s_suspend_pass_counter = 0;

    startup_apply_led_driver_state();

    g_keys_last_pressed_key_id = k_keys_no_key_id;
    if (g_adc_scan_cycle_busy == 0) {
        adc_scan_start_cycle();
    }
    keys_process_scan_pass();

    if (g_keys_last_pressed_key_id != KEYS_KEY_ID_NONE
        && (g_run_state_flags & RUN_STATE_FLAG_REMOTE_WAKEUP) != 0) {
        /* tud_remote_wakeup acts only while the bus is suspended and the host has enabled
         * remote wakeup; it then signals resume on the bus for the fixed busy loop of the
         * USB driver (the specification asks for 1 to 15 ms) and watches for the start of
         * frame packets with which the host resumes. */
        tud_remote_wakeup();
    }
}

/*
 * Main-loop body in normal operation, once per pass: hand the finished scan cycle to
 * the key logic (or, while the scan settles after a mode switch, only wait for the
 * cycle to end), step the key-rate window, run a deferred profile switch, follow the
 * LED driver on/off state, render an rgb frame and start the next scan cycle.
 */
void startup_run_active_task(void)
{
    if (g_adc_scan_cycle_busy == 0) {
        adc_scan_start_cycle();
    }

    if (g_adc_scan_settling != 0) {
        while (g_adc_scan_event_scan_end == ADC_SCAN_EVENT_INDEX_NONE) {
            tud_task();
        }
    } else {
        keys_process_scan_pass();
    }

    signals_key_rate_step();

    if (g_profile_switch_pending != 0) {
        profile_activate(g_profile_switch_target.raw);
        g_profile_switch_pending = 0;
    }

    startup_apply_led_driver_state();
    if (g_rgb_driver_enabled != 0) {
        rgb_update_frame();
    }

    adc_scan_start_cycle();
}

/*
 * Sends the "profiles changed" event requested by protocol_delete_last_profile
 * once the TX ring has a free entry. */
void protocol_notify_service(void)
{
    if (g_protocol_notify_pending != 0 && g_protocol_tx_ring.count <= PROTOCOL_TX_RING_SIZE - 1u) {
        protocol_send_profiles_changed_event();
    }
}

/* Marks the HID OUT report buffer as consumed, with interrupts masked. */
static void protocol_release_rx_report(void)
{
    uint32_t irq_state = hal_irq_save();

    g_protocol_rx_report = NULL;
    hal_irq_restore(irq_state);
}

/*
 * Executes at most one entry of g_protocol_cmd_queue (filled by
 * tud_hid_set_report_cb). A feature command answers on the IN endpoint and is therefore left in
 * the queue while the TX ring is full; an OUT-report command has no response. Every
 * queue access masks interrupts, as on the producer side.
 */
void protocol_queue_service(void)
{
    protocol_cmd_queue_entry_t entry = { 0 };
    uint32_t irq_state;
    uint8_t index;

    /* peek at the oldest entry */
    irq_state = hal_irq_save();
    if (g_protocol_cmd_queue_count == 0) {
        hal_irq_restore(irq_state);
        return;
    }
    entry = g_protocol_cmd_queue[g_protocol_cmd_queue_rd];
    hal_irq_restore(irq_state);

    if (entry.type == PROTOCOL_CMD_TYPE_FEATURE && g_protocol_tx_ring.count > PROTOCOL_TX_RING_SIZE - 1u) {
        return;
    }

    /* pop it */
    irq_state = hal_irq_save();
    if (g_protocol_cmd_queue_count == 0) {
        hal_irq_restore(irq_state);
        return;
    }
    index = g_protocol_cmd_queue_rd;
    g_protocol_cmd_queue_rd++;
    if (g_protocol_cmd_queue_rd == PROTOCOL_CMD_QUEUE_SIZE) {
        g_protocol_cmd_queue_rd = 0;
    }
    entry = g_protocol_cmd_queue[index];
    g_protocol_cmd_queue_count--;
    hal_irq_restore(irq_state);

    if (entry.type == PROTOCOL_CMD_TYPE_FEATURE) {
        protocol_handle_feature_command(&entry.feature);
        return;
    }

    /* OUT report: still in the HID OUT buffer of the protocol interface */
    if (g_protocol_rx_report != NULL) {
        protocol_handle_report_command(g_protocol_rx_report);
    }
    protocol_release_rx_report();

    /* Give the OUT endpoint back to the host: look up the open HID instance
     * of interface 2 and, if it has an OUT endpoint, arm it for the next 2048 bytes. */
    tud_hid_n_receive_rearm(tud_hid_itf_num_to_instance(USB_ITF_PROTOCOL));
}
