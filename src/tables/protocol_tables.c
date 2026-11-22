/* Constant tables of module `protocol` (definitions). */
#include "protocol_tables.h"

/* Reply code of feature command 69 by TinyUSB speed (0 full -> 2, 1 low -> 1, 2 high -> 3) */
const uint8_t k_protocol_usb_speed_codes[3] = {
    2, 1, 3,
};

/* Mux channel per [mux][step], read only by protocol_cmd_get_key_sensor_values kind 3; all zero
 * on every board, sized by the board's scan numbers. Matches stock firmware 2.14.1. */
const uint8_t k_protocol_sensor_scan_order[BOARD_MUX_COUNT][BOARD_MUX_ADDRESSES_USED] = { { 0 } };
