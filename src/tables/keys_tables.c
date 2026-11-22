/* Constant tables of module `keys` (definitions). */
#include "keys_tables.h"

/* 0xFF = "no key": value loaded into g_keys_last_pressed_key_id by startup_run_suspended_task */
const uint8_t k_keys_no_key_id = 0xFF;

/* Tap length in ms per g_keys_tap_duration_preset 0..2: 1, 20, 40 (keys_process_scan_pass; other
 * presets use 40) */
const uint8_t k_keys_tap_duration_ms[3] = {
     1, 20, 40,
};
