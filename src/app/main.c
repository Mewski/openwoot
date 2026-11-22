/*
 * app: main, the start-up and the main loop of the firmware.
 *
 * main runs the start-up steps that the modules register (core/startup.h gives their
 * order), then the main loop, which never returns.
 *
 * Concurrency: three interrupt sources exist. The 1 ms tick (hal_time_tick_isr, lowest
 * priority, from the first start-up step on), the end of a sensor conversion
 * (hal_scan_conversion_isr, from the start-up sweep on) and the USB controller (from the
 * start of the USB stack on). Everything else, including all USB class callbacks, runs in
 * the main loop.
 *
 * Depends on: hal_system, TinyUSB, core/clock.h, core/run_state.h, core/soft_timer.h,
 * core/startup.h, app/tasks.h, and the protocol and usb modules whose services the loop
 * runs.
 */
#include "hal/hal_system.h"
#include "tusb.h"

#include "app/tasks.h"
#include "core/clock.h"
#include "core/run_state.h"
#include "core/soft_timer.h"
#include "core/startup.h"
#include "protocol/protocol.h"
#include "usb/usb.h"

/* Time from a restart request to the reset, during which the device stays disconnected. */
#define MAIN_RESTART_DELAY_MS           3000u

/* The shared set-up of the microcontroller is the first step after the tick. */
STARTUP_STEP(STARTUP_SYSTEM, hal_system_init);

/* Runs every registered start-up step, in ascending order number. */
static void startup_run_steps(void)
{
    REGISTRY_FOREACH_ORDERED(startup_steps, startup_step_t, step) {
        step->run();
    }
}

/* Runs the start-up steps, then the main loop; never returns. */
int main(void)
{
    uint32_t restart_stamp;

    startup_run_steps();

    restart_stamp = 0;
    for (;;) {
        uint8_t run_flags;

        tud_task();

        if ((g_usb_link_flags & USB_LINK_SUSPENDED) != 0) {
            startup_run_suspended_task();
        } else {
            startup_run_active_task();
        }

        soft_timers_service();
        protocol_notify_service();
        protocol_queue_service();
        protocol_flush_tx_ring();

        /* Restart: a request bit starts a countdown during which the device
         * stays disconnected (tud_disconnect on every pass); after
         * MAIN_RESTART_DELAY_MS the microcontroller is reset. Nothing in this firmware
         * sets the request bit. If both the request and the run bit are clear without a
         * request (not reachable through the code above), the countdown uses the old stamp. */
        run_flags = g_run_state_flags;
        if ((run_flags & RUN_STATE_FLAG_RESTART) != 0) {
            restart_stamp = g_clock_uptime_ms;
            g_run_state_flags = run_flags & (uint8_t)~(RUN_STATE_FLAG_RESTART | RUN_STATE_FLAG_RUN);
        } else if ((run_flags & RUN_STATE_FLAG_RUN) != 0) {
            continue;
        }

        tud_disconnect();
        if (g_clock_uptime_ms - restart_stamp > MAIN_RESTART_DELAY_MS) {
            g_run_state_flags |= RUN_STATE_FLAG_RUN;
            hal_system_reset();
        }
    }
}
