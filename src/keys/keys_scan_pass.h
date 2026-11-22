/* Public function of the scan pass: turns the analog key states into the HID reports. */
#ifndef KEYS_KEYS_SCAN_PASS_H_
#define KEYS_KEYS_SCAN_PASS_H_

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "keys/keys_types.h"
#include "keys/keys_data.h"

void keys_process_scan_pass(void);

#endif /* KEYS_KEYS_SCAN_PASS_H_ */
