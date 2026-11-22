/*
 * core: the run state of the firmware, two bytes of flags that many modules test: the run
 * flags (remote wakeup allowed, keyboard report format, restart request, scan mode level)
 * and the option flags (raw scanning, forced USB full speed). Each bit names the code
 * that sets it.
 *
 * Depends on: nothing.
 */
#ifndef CORE_RUN_STATE_H_
#define CORE_RUN_STATE_H_

#include <stdint.h>

/* g_run_state_flags (one byte of bit fields) */
#define RUN_STATE_FLAG_REMOTE_WAKEUP  0x01u /* bit 0: host enabled remote wakeup (tud_task_ext, tud_suspend_cb); tested by startup_run_suspended_task */
#define RUN_STATE_FLAG_6KRO           0x02u /* bit 1: 6KRO keyboard report descriptor selected by the device settings (settings_load_and_apply); bcdDevice bit 0 (usb_descriptors_init) */
#define RUN_STATE_FLAG_6KRO_ACTIVE    0x04u /* bit 2: keyboard reports use the 8-byte boot / 6KRO format: 1 while the keyboard interface is closed or in boot protocol, else a copy of bit 1 (updated in tud_mount_cb, tud_umount_cb, hidd_control_xfer_cb); read by keys_process_scan_pass */
#define RUN_STATE_FLAG_RESTART        0x08u /* bit 3: restart request, polled by the main loop */
#define RUN_STATE_FLAG_RUN            0x10u /* bit 4: normal operation; cleared with bit 3 for the restart countdown, set again just before the reset */
#define RUN_STATE_SCAN_MODE_SHIFT     5u
#define RUN_STATE_SCAN_MODE_MASK      0xE0u /* bits 7:5: scan mode level from the profile cfg (profile_apply_cfg), 0 or 1 */

#define RUN_STATE_FLAGS_INIT          RUN_STATE_FLAG_RUN /* .data initial value 0x10 */

/* g_run_state_options */
#define RUN_STATE_OPTION_RAW_SCANNING    0x01u /* bit 0: raw scanning requested by feature command 44; read by the ADC scan paths */
#define RUN_STATE_OPTION_USB_FULL_SPEED  0x02u /* bit 1: force USB full speed, from the device settings (settings_load_and_apply, usb_speed_option_init) */

extern uint8_t g_run_state_flags;     /* run flags, RUN_STATE_FLAG_* and RUN_STATE_SCAN_MODE_* */
extern uint8_t g_run_state_options;   /* option flags, RUN_STATE_OPTION_* */

#endif /* CORE_RUN_STATE_H_ */
